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
            StatusCode::NO_CONTENT => Err(DownloadError::JobFailedOrCleaned),
            StatusCode::NOT_FOUND => Err(DownloadError::JobNotFound),
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
}
