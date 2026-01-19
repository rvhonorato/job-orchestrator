use crate::config::loader::Config;
use crate::models::job_dao::Job;
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
    #[error("Job not found")]
    JobNotFound,
    #[error("Job not ready yet")]
    JobNotReady,
    #[error("Job failed during execution")]
    JobFailed,
    #[error("Job results cleaned up (expired)")]
    JobCleaned,
    #[error("Job invalid (user error)")]
    JobInvalid,
    #[error("Server returned error status {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },
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

pub async fn retrieve<T>(job: &Job, config: &Config, target: T) -> Result<(), DownloadError>
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

// pub async fn status() {}

// These are traits that all Desinations need to have
pub trait Endpoint {
    async fn upload(&self, j: &Job, url: &str) -> Result<u32, UploadError>;
    // async fn status(&self, j: &Job) -> Result<reqwest::Response, reqwest::Error>;
    async fn download(&self, j: &Job, url: &str) -> Result<(), DownloadError>;
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::config::loader::Service;
    use crate::models::job_dao::Job;
    use std::collections::HashMap;
    use std::time::Duration;
    use tempfile::TempDir;
    use uuid::Uuid;

    // Mock `upload` and `download`,
    //  the tests below are testing just the logic of the
    //  `send` and `retrieve` logic
    struct OkMockDestination;
    struct ErrMockDestination;

    impl Endpoint for OkMockDestination {
        async fn upload(&self, _j: &Job, _u: &str) -> Result<u32, UploadError> {
            Ok(0)
        }
        async fn download(&self, _j: &Job, _u: &str) -> Result<(), DownloadError> {
            Ok(())
        }
    }

    impl Endpoint for ErrMockDestination {
        async fn upload(&self, _j: &Job, _u: &str) -> Result<u32, UploadError> {
            Err(UploadError::InvalidService)
        }
        async fn download(&self, _j: &Job, _u: &str) -> Result<(), DownloadError> {
            Err(DownloadError::InvalidService)
        }
    }

    #[tokio::test]
    async fn test_send_ok() {
        let service_name = Uuid::new_v4().to_string();
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.service = service_name.clone();

        let mut services = HashMap::new();
        services.insert(
            service_name.clone(),
            Service {
                name: service_name,
                upload_url: "".to_string(),
                download_url: "".to_string(),
                runs_per_user: 5,
            },
        );
        let config = Config {
            services,
            data_path: "".to_string(),
            db_path: "".to_string(),
            max_age: Duration::from_secs(1),
            port: 1111,
        };
        let target = OkMockDestination;

        let result = send(&job, &config, target).await;
        assert!(result.is_ok());

        let target = ErrMockDestination;
        let result = send(&job, &config, target).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_err() {
        let service_name = Uuid::new_v4().to_string();
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.service = service_name.clone();

        let mut services = HashMap::new();
        services.insert(
            service_name.clone(),
            Service {
                name: service_name,
                upload_url: "".to_string(),
                download_url: "".to_string(),
                runs_per_user: 5,
            },
        );
        let config = Config {
            services,
            data_path: "".to_string(),
            db_path: "".to_string(),
            max_age: Duration::from_secs(1),
            port: 1111,
        };

        let target = ErrMockDestination;
        let result = send(&job, &config, target).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retrieve_ok() {
        let service_name = Uuid::new_v4().to_string();
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.service = service_name.clone();
        job.id = 42;

        let mut services = HashMap::new();
        services.insert(
            service_name.clone(),
            Service {
                name: service_name,
                upload_url: "".to_string(),
                download_url: "".to_string(),
                runs_per_user: 5,
            },
        );
        let config = Config {
            services,
            data_path: "".to_string(),
            db_path: "".to_string(),
            max_age: Duration::from_secs(1),
            port: 1111,
        };
        let target = OkMockDestination;

        let result = retrieve(&job, &config, target).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_retrieve_err() {
        let service_name = Uuid::new_v4().to_string();
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.service = service_name.clone();
        job.id = 42;

        let mut services = HashMap::new();
        services.insert(
            service_name.clone(),
            Service {
                name: service_name,
                upload_url: "".to_string(),
                download_url: "".to_string(),
                runs_per_user: 5,
            },
        );
        let config = Config {
            services,
            data_path: "".to_string(),
            db_path: "".to_string(),
            max_age: Duration::from_secs(1),
            port: 1111,
        };
        let target = ErrMockDestination;

        let result = retrieve(&job, &config, target).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retrieve_err_empty_job() {
        let tempdir = TempDir::new().unwrap();
        let job = Job::new(tempdir.path().to_str().unwrap());

        let mut services = HashMap::new();
        services.insert(
            "".to_string(),
            Service {
                name: "".to_string(),
                upload_url: "".to_string(),
                download_url: "".to_string(),
                runs_per_user: 5,
            },
        );
        let config = Config {
            services,
            data_path: "".to_string(),
            db_path: "".to_string(),
            max_age: Duration::from_secs(1),
            port: 1111,
        };
        let target = ErrMockDestination;

        let result = retrieve(&job, &config, target).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(DownloadError::NotFound)))
    }
}
