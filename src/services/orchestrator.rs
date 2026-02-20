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

// pub async fn status() {}

// These are traits that all Desinations need to have
pub trait Endpoint {
    async fn upload(&self, j: &Job, url: &str) -> Result<u32, UploadError>;
    // async fn status(&self, j: &Job) -> Result<reqwest::Response, reqwest::Error>;
    async fn download(&self, j: &Job, url: &str) -> Result<Status, DownloadError>;
}
