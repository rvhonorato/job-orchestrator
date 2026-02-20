use crate::config::loader::Config;
use crate::models::job_dao::Job;
use crate::models::status_dto::Status;
use anyhow::Result;
use axum::http::StatusCode;
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("Invalid service")]
    InvalidService,
    #[error("Failed to encode file: {0}")]
    EncodingFailed(#[from] std::io::Error),
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Failed to read response: {0}")]
    ResponseReadFailed(reqwest::Error),
    #[error("Failed to deserialize response: {0}")]
    DeserializationFailed(#[from] serde_json::Error),
    #[error("Server returned error status {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },
    #[error("Cannot read file '{path}': {source}")]
    FileRead {
        path: String,
        #[source]
        source: tokio::io::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Failed to read response: {0}")]
    ResponseReadFailed(reqwest::Error),
    #[error("Failed to create file '{path}': {source}")]
    FileCreate {
        path: String,
        #[source]
        source: tokio::io::Error,
    },
    #[error("Failed to write to file '{path}': {source}")]
    FileWrite {
        path: String,
        #[source]
        source: tokio::io::Error,
    },
    #[error("Not found")]
    NotFound,
    #[error("Invalid service")]
    InvalidService,
}

pub async fn send<T>(job: &Job, config: &Config, target: T) -> Result<u32, UploadError>
where
    T: Endpoint,
{
    info!("{:?}", job);

    match config.get_upload_url(&job.service) {
        Some(url) => Ok(target.upload(job, url).await?),
        None => Err(UploadError::InvalidService),
    }
}

pub async fn retrieve<T>(job: &Job, config: &Config, target: T) -> Result<Status, DownloadError>
where
    T: Endpoint,
{
    if job.id == 0 {
        Err(DownloadError::NotFound)
    } else {
        // target.download(job).await
        match config.get_download_url(&job.service) {
            Some(url) => Ok(target.download(job, url).await?),
            None => Err(DownloadError::InvalidService),
        }
    }
}

// These are traits that all Destinations need to have
pub trait Endpoint {
    async fn upload(&self, j: &Job, url: &str) -> Result<u32, UploadError>;
    async fn download(&self, j: &Job, url: &str) -> Result<Status, DownloadError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::{Config, Service};
    use crate::models::job_dao::Job;
    use crate::models::status_dto::Status;
    use std::collections::HashMap;
    use tempfile::TempDir;

    struct OkMockEndpoint;
    struct ErrMockEndpoint;

    impl Endpoint for OkMockEndpoint {
        async fn upload(&self, _j: &Job, _url: &str) -> Result<u32, UploadError> {
            Ok(42)
        }
        async fn download(&self, _j: &Job, _url: &str) -> Result<Status, DownloadError> {
            Ok(Status::Completed)
        }
    }

    impl Endpoint for ErrMockEndpoint {
        async fn upload(&self, _j: &Job, _url: &str) -> Result<u32, UploadError> {
            Err(UploadError::InvalidService)
        }
        async fn download(&self, _j: &Job, _url: &str) -> Result<Status, DownloadError> {
            Err(DownloadError::NotFound)
        }
    }

    fn make_config() -> Config {
        let mut services = HashMap::new();
        services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                runs_per_user: 5,
            },
        );
        Config {
            services,
            db_path: "/tmp/test.db".to_string(),
            data_path: "/tmp".to_string(),
            max_age: std::time::Duration::from_secs(3600),
            port: 5000,
        }
    }

    fn make_job(data_path: &str, service: &str, id: i32) -> Job {
        let mut job = Job::new(data_path);
        job.set_service(service.to_string());
        job.id = id;
        job
    }

    #[tokio::test]
    async fn test_send_valid_service() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "test", 1);
        let result = send(&job, &config, OkMockEndpoint).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_send_invalid_service() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "nonexistent", 1);
        let result = send(&job, &config, OkMockEndpoint).await;
        assert!(matches!(result.unwrap_err(), UploadError::InvalidService));
    }

    #[tokio::test]
    async fn test_send_propagates_error() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "test", 1);
        let result = send(&job, &config, ErrMockEndpoint).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retrieve_valid_job() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "test", 1);
        let result = retrieve(&job, &config, OkMockEndpoint).await;
        assert_eq!(result.unwrap(), Status::Completed);
    }

    #[tokio::test]
    async fn test_retrieve_job_id_zero() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "test", 0);
        let result = retrieve(&job, &config, OkMockEndpoint).await;
        assert!(matches!(result.unwrap_err(), DownloadError::NotFound));
    }

    #[tokio::test]
    async fn test_retrieve_invalid_service() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "nonexistent", 1);
        let result = retrieve(&job, &config, OkMockEndpoint).await;
        assert!(matches!(result.unwrap_err(), DownloadError::InvalidService));
    }

    #[tokio::test]
    async fn test_retrieve_propagates_error() {
        let tempdir = TempDir::new().unwrap();
        let config = make_config();
        let job = make_job(tempdir.path().to_str().unwrap(), "test", 1);
        let result = retrieve(&job, &config, ErrMockEndpoint).await;
        assert!(result.is_err());
    }
}
