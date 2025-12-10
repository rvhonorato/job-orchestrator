use axum::http::StatusCode;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    #[cfg(target_os = "windows")]
    fn test_sanitize_filename_windows_path() {
        assert_eq!(sanitize_filename("C:\\Windows\\file.txt"), "file.txt");
        assert_eq!(sanitize_filename("D:\\Documents\\test.doc"), "test.doc");
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
