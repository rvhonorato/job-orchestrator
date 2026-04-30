use crate::models::status_dto::Status;

use crate::models::job_dao::Job;
use crate::models::payload_dao::Payload;
use crate::services::endpoint::{DownloadError, UploadError};
use crate::services::endpoint::{Endpoint, TerminateError};
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use walkdir::WalkDir;

use crate::config::loader::Config;
use crate::models::queue_dao::PayloadQueue;
use axum::http::{StatusCode, header};
use sqlx::SqlitePool;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Execution error")]
    Execution,
    #[error("No execution script found")]
    NoExecScript,
    #[error("Unsafe script detected: {reason}")]
    UnsafeScript { reason: String },
    #[error("Missing requirement: {reason}")]
    MissingRequirement { reason: String },
}

pub struct Client;

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
        } else if status.is_success() {
            // Job not yet finished, propagate the status
            let payload: Payload = match response.json().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(DownloadError::ResponseReadFailed(e));
                }
            };
            Ok(payload.status)
        } else {
            // Client returned an error
            tracing::error!("Client returned error status: {status}");
            Err(DownloadError::RequestFailed(
                response.error_for_status().unwrap_err(),
            ))
        }
    }

    async fn terminate(&self, j: &Job, url: &str) -> Result<(), TerminateError> {
        // Make the request to the client
        let client = reqwest::Client::new();
        let response = client.post(format!("{url}/{0}", j.dest_id)).send().await;

        match response {
            Ok(r) => {
                if r.status().is_success() {
                    // job was terminated - all ok
                    Ok(())
                } else {
                    Err(TerminateError::HttpError(r.status()))
                }
            }
            Err(e) => {
                error!("termination request failed: {}", e);
                Err(TerminateError::GenericError)
            }
        }
    }
}

// Runner will spawn the processes in the background
pub async fn runner(pool: SqlitePool, config: Config) {
    let mut queue = PayloadQueue::new(&config);
    if queue.list_per_status(Status::Prepared, &pool).await.is_ok() {
        let futures = queue
            .jobs
            .into_iter()
            .map(|mut payload| {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    // Mark the job as running, without this status it will stay in `Processing`
                    payload
                        .update_status(Status::Running, &pool_clone)
                        .await
                        .ok();

                    if let Err(e) = payload.execute() {
                        // There was some error in execution
                        error!("There was an error while executing the payload: {e}");
                        let status = match e {
                            // Some script  error, mark as invalid
                            // TODO: Figure out a way to propagate this error to the user
                            ClientError::NoExecScript
                            | ClientError::UnsafeScript { .. }
                            | ClientError::MissingRequirement { .. } => Status::Invalid,
                            // Some error during process spawn
                            ClientError::Execution => Status::Failed,
                        };
                        // Here job will be either INVALID or FAILED
                        payload.update_status(status, &pool_clone).await.ok();
                    } else {
                        // Process was spawned, add `pid` to database
                        payload.update_pid(&pool_clone).await.ok();
                        // Don't change the status, it's already running
                    }
                })
            })
            .collect::<Vec<_>>();

        futures::future::join_all(futures).await;
    }
}

// Updater will go over the Running jobs and check their exis status
pub async fn updater(pool: SqlitePool, config: Config) {
    let mut queue = PayloadQueue::new(&config);
    if queue.list_per_status(Status::Running, &pool).await.is_ok() {
        let futures = queue
            .jobs
            .into_iter()
            .map(|mut j| {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    // NOTE: Order here is important!
                    // PID can be re-used by the system, so we can only rely on it
                    // IF the exit flag is not present
                    if j.is_killed() {
                        j.update_status(Status::Killed, &pool_clone).await.ok();
                    } else if j.is_exit()
                        && let Some(status_code) = j.status_code()
                    {
                        if status_code == 0 {
                            j.update_status(Status::Completed, &pool_clone).await.ok();
                        } else {
                            j.update_status(Status::Failed, &pool_clone).await.ok();
                        }
                    } else if j.is_running() == Some(true) {
                        //  DO NOTHING
                        // NOTE: PID is actually running, do nothing - this branch needs to exist
                    } else {
                        // DO NOTHING
                        // NOTE: If it reached this condition, the payload can be either lost
                        //  or is in a race condition writing the file.
                        // To avoid the race we keep the payload running
                        // With the trade-off that if the job is truly gone
                        //  there is no way of capturing it.
                    }
                })
            })
            .collect::<Vec<_>>();

        futures::future::join_all(futures).await;
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use mockito::Server;
    use std::fs;
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_client_download_non_completed() {
        let mut server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();

        let mut job = Job::new(temp_dir.path().to_str().unwrap());
        job.dest_id = 99;
        fs::create_dir_all(&job.loc).unwrap();

        // Server returns 200 with JSON Payload — job still running
        let mut running_payload = Payload::new();
        running_payload.set_id(99);
        running_payload.set_status(crate::models::status_dto::Status::Running);
        let body = serde_json::to_string(&running_payload).unwrap();

        let mock = server
            .mock("GET", "/retrieve/99")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let client = Client;
        let url = format!("{}/retrieve", server.url());
        let result = client.download(&job, &url).await;

        mock.assert_async().await;
        assert_eq!(result.unwrap(), crate::models::status_dto::Status::Running);
    }

    #[tokio::test]
    async fn test_runner() {
        // Initialize pool
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        // Initialize config
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Add a payload
        let mut payload = Payload::new();
        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        // Add input data
        let data = b"#!/bin/bash\ntrap 'echo $? > .orchestrator.exit' EXIT\necho 'Hello, World!' > output.txt\n";
        payload.add_input("run.sh".to_string(), data.to_vec());

        // Prepare the payload
        payload
            .prepare(&config.data_path)
            .expect("Failed to prepare payload");

        // Update loc in database after prepare
        payload
            .update_loc(&pool)
            .await
            .expect("Failed to update payload loc");

        // Mark as prepared
        payload
            .update_status(Status::Prepared, &pool)
            .await
            .expect("Failed to update payload status");

        assert!(payload.loc.exists());

        // Run the runner
        runner(pool.clone(), config).await;

        // Check the effects
        let mut _payload = Payload::retrieve_id(payload.id, &pool)
            .await
            .expect("Failed to retrieve payload");

        assert_eq!(_payload.status, Status::Running);
    }

    /// When run.sh is missing, the job should be marked as Invalid (user error).
    #[tokio::test]
    async fn test_runner_no_script_sets_invalid() {
        // Initialize pool
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        // Initialize config with tempdir
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Add a payload WITHOUT a run.sh script
        let mut payload = Payload::new();
        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        // Add some other file, but NOT run.sh
        payload.add_input("data.txt".to_string(), b"some data".to_vec());

        // Prepare the payload
        payload
            .prepare(&config.data_path)
            .expect("Failed to prepare payload");

        // Update loc in database after prepare
        payload
            .update_loc(&pool)
            .await
            .expect("Failed to update payload loc");

        // Mark as prepared
        payload
            .update_status(Status::Prepared, &pool)
            .await
            .expect("Failed to update payload status");

        // Run the runner with the same config
        runner(pool.clone(), config).await;

        // Check the effects
        let _payload = Payload::retrieve_id(payload.id, &pool)
            .await
            .expect("Failed to retrieve payload");

        // No run.sh = user error = Invalid status
        assert_eq!(_payload.status, Status::Invalid);
    }

    #[tokio::test]
    async fn test_updater_killed_status() {
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Create a payload in Running state with a fake PID
        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload.set_loc(tempdir.path().join(payload.id.to_string()));
        payload.update_loc(&pool).await.unwrap();
        payload.pid = 999999; // Fake PID that doesn't exist
        payload.update_pid(&pool).await.unwrap();
        payload.update_status(Status::Running, &pool).await.unwrap();

        // Mark as killed
        payload.mark_as_killed(&pool).await.unwrap();

        // Run the updater
        updater(pool.clone(), config).await;

        // Verify status was updated to Killed
        let retrieved = Payload::retrieve_id(payload.id, &pool).await.unwrap();
        assert_eq!(retrieved.status, Status::Killed);
    }

    #[tokio::test]
    async fn test_updater_failed_no_exit_file() {
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Create a payload in Running state with a fake PID
        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload.set_loc(tempdir.path().join(payload.id.to_string()));
        fs::create_dir_all(&payload.loc).unwrap();
        payload.update_loc(&pool).await.unwrap();
        payload.pid = 999999; // Fake PID that killed/crashed
        payload.update_pid(&pool).await.unwrap();
        payload.update_status(Status::Running, &pool).await.unwrap();

        // Run the updater - should remain Running since no exit file exists yet
        updater(pool.clone(), config).await;

        // Verify status remains Running (race condition: exit file may appear next cycle)
        let retrieved = Payload::retrieve_id(payload.id, &pool).await.unwrap();
        assert_eq!(retrieved.status, Status::Running);
    }

    #[tokio::test]
    async fn test_updater_completed_with_exit_code_zero() {
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Create a payload in Running state with a fake PID and exit file with code 0
        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload.set_loc(tempdir.path().join(payload.id.to_string()));
        fs::create_dir_all(&payload.loc).unwrap();
        payload.update_loc(&pool).await.unwrap();
        payload.pid = 999999; // Fake PID
        payload.update_pid(&pool).await.unwrap();
        payload.update_status(Status::Running, &pool).await.unwrap();

        // Create exit file with code 0
        fs::write(payload.loc.join(".orchestrator.exit"), "0").unwrap();

        // Run the updater
        updater(pool.clone(), config).await;

        // Verify status was updated to Completed
        let retrieved = Payload::retrieve_id(payload.id, &pool).await.unwrap();
        assert_eq!(retrieved.status, Status::Completed);
    }

    #[tokio::test]
    async fn test_updater_failed_with_nonzero_exit_code() {
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;
        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Create a payload in Running state with a fake PID and exit file with code 1
        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload.set_loc(tempdir.path().join(payload.id.to_string()));
        fs::create_dir_all(&payload.loc).unwrap();
        payload.update_loc(&pool).await.unwrap();
        payload.pid = 999999; // Fake PID
        payload.update_pid(&pool).await.unwrap();
        payload.update_status(Status::Running, &pool).await.unwrap();

        // Create exit file with code 1
        fs::write(payload.loc.join(".orchestrator.exit"), "1").unwrap();

        // Run the updater
        updater(pool.clone(), config).await;

        // Verify status was updated to Failed
        let retrieved = Payload::retrieve_id(payload.id, &pool).await.unwrap();
        assert_eq!(retrieved.status, Status::Failed);
    }

    // ===== terminate() tests =====

    #[tokio::test]
    async fn test_terminate_success() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/123")
            .with_status(200)
            .create_async()
            .await;

        let client = Client;
        let mut job = Job::new("/tmp");
        job.dest_id = 123;

        let result = client.terminate(&job, &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_terminate_http_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/456")
            .with_status(500)
            .create_async()
            .await;

        let client = Client;
        let mut job = Job::new("/tmp");
        job.dest_id = 456;

        let result = client.terminate(&job, &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TerminateError::HttpError(status) => assert_eq!(status, 500),
            _ => panic!("Expected HttpError"),
        }
    }

    #[tokio::test]
    async fn test_terminate_request_error() {
        let client = Client;
        let mut job = Job::new("/tmp");
        job.dest_id = 789;

        // Use an invalid URL that will cause a connection error
        let result = client
            .terminate(&job, "http://invalid-url-that-does-not-exist-12345.com")
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TerminateError::GenericError => {}
            _ => panic!("Expected GenericError"),
        }
    }
}
