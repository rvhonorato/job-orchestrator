use std::process::Command;

use crate::models::job_dao::Job;
use crate::models::payload_dao::Payload;
use crate::services::orchestrator::Endpoint;
use crate::services::orchestrator::{DownloadError, UploadError};
use futures_util::StreamExt;
use http::StatusCode;
use reqwest::multipart::{Form, Part};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use tracing::info;
use walkdir::WalkDir;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Execution error")]
    Execution,
    #[error("Script error")]
    Script,
    #[error("No execution script found")]
    NoExecScript,
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

    async fn download(&self, j: &Job, url: &str) -> Result<(), DownloadError> {
        let client = reqwest::Client::new();
        // Append the job id to the url
        let response = client
            .get(format!("{url}/{0}", j.dest_id))
            .send()
            .await
            .map_err(DownloadError::RequestFailed)?;

        let status = response.status();

        match status {
            StatusCode::OK => {
                let output_path = j.loc.join("output.zip");
                let mut file =
                    File::create(&output_path)
                        .await
                        .map_err(|e| DownloadError::FileCreate {
                            path: output_path.display().to_string(),
                            source: e,
                        })?;

                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(DownloadError::ResponseReadFailed)?;
                    file.write_all(&chunk)
                        .await
                        .map_err(|e| DownloadError::FileWrite {
                            path: output_path.display().to_string(),
                            source: e,
                        })?;
                }
                file.flush().await.map_err(|e| DownloadError::FileWrite {
                    path: output_path.display().to_string(),
                    source: e,
                })?;

                Ok(())
            }
            StatusCode::ACCEPTED => Err(DownloadError::JobNotReady),
            StatusCode::NO_CONTENT => Err(DownloadError::JobCleaned),
            StatusCode::BAD_REQUEST => Err(DownloadError::JobInvalid),
            StatusCode::NOT_FOUND => Err(DownloadError::JobNotFound),
            StatusCode::GONE => Err(DownloadError::JobFailed),
            StatusCode::INTERNAL_SERVER_ERROR => Err(DownloadError::JobFailed),
            _ => {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unable to read response body".to_string());
                Err(DownloadError::UnexpectedStatus { status, body })
            }
        }
    }
}

// Client side
pub fn execute_payload(payload: &Payload) -> Result<(), ClientError> {
    info!("{:?}", payload);

    // Expect the payload.loc to contain a `run.sh` script
    let run_script = payload.loc.join("run.sh");

    // Make sure the script exists
    if !run_script.exists() {
        return Err(ClientError::NoExecScript);
    }

    // Execute script and wait for it to finish
    let exit_status = Command::new("bash")
        .arg(run_script)
        .current_dir(&payload.loc)
        .status()
        .map_err(|_| ClientError::Execution)?;

    if !exit_status.success() {
        return Err(ClientError::Script);
    }

    Ok(())
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

    #[test]
    fn test_execute_payload_script_error() {
        // Prepare a temporary payload
        let temp_dir = tempfile::tempdir().unwrap();
        let mut payload = Payload::new();
        payload.set_loc(temp_dir.path().to_path_buf());

        // Add a run.sh script that fails
        std::fs::write(payload.loc.join("run.sh"), b"#!/bin/bash\nexit 1").unwrap();

        let result = execute_payload(&payload);

        assert!(matches!(result, Err(ClientError::Script)));
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

    #[tokio::test]
    async fn test_client_download_accepted() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 456;
        fs::create_dir_all(&job.loc).unwrap();

        // Mock server response with ACCEPTED status
        let mock = server
            .mock("GET", "/retrieve/456")
            .with_status(202)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DownloadError::JobNotReady)));
    }

    #[tokio::test]
    async fn test_client_download_no_content() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 789;
        fs::create_dir_all(&job.loc).unwrap();

        // Mock server response with NO_CONTENT status (job results cleaned/expired)
        let mock = server
            .mock("GET", "/retrieve/789")
            .with_status(204)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DownloadError::JobCleaned)));
    }

    #[tokio::test]
    async fn test_client_download_not_found() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 999;
        fs::create_dir_all(&job.loc).unwrap();

        // Mock server response with NOT_FOUND status
        let mock = server
            .mock("GET", "/retrieve/999")
            .with_status(404)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DownloadError::JobNotFound)));
    }

    #[tokio::test]
    async fn test_client_download_unexpected_status() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 111;
        fs::create_dir_all(&job.loc).unwrap();

        // Mock server response with unexpected status
        let mock = server
            .mock("GET", "/retrieve/111")
            .with_status(418) // I'm a teapot
            .with_body("Unexpected error")
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result {
            Err(DownloadError::UnexpectedStatus { status, body }) => {
                assert_eq!(status, StatusCode::IM_A_TEAPOT);
                assert_eq!(body, "Unexpected error");
            }
            _ => panic!("Expected UnexpectedStatus error"),
        }
    }

    #[tokio::test]
    async fn test_client_download_large_file() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 222;
        fs::create_dir_all(&job.loc).unwrap();

        // Create large content (1MB)
        let large_content = vec![b'A'; 1024 * 1024];

        let mock = server
            .mock("GET", "/retrieve/222")
            .with_status(200)
            .with_body(&large_content)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        // Verify file size
        let output_path = job.loc.join("output.zip");
        let metadata = fs::metadata(output_path).unwrap();
        assert_eq!(metadata.len(), 1024 * 1024);
    }
}
