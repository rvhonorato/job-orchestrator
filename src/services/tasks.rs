use std::fs;
use std::time::SystemTime;

use crate::config::loader::Config;
use crate::models::job_dao::Job;
use crate::models::queue_dao::PayloadQueue;
use crate::models::{queue_dao::Queue, status_dto::Status};
use crate::services::client::{execute_payload, Client};
use crate::services::orchestrator;
use futures::stream::{self, StreamExt};
use sqlx::SqlitePool;
use tracing::info;
use tracing::{debug, error, warn};

use super::client::ClientError;
use super::orchestrator::DownloadError;

pub async fn cleaner(pool: SqlitePool, config: Config) {
    // List all directories inside the config.data_path
    let elements = match fs::read_dir(&config.data_path) {
        Ok(e) => e,
        Err(_) => {
            error!("could not read directory: {}", config.data_path);
            return;
        }
    };

    let futures = elements.into_iter().map(|entry| async {
        let entry = match entry {
            Ok(d) => d,
            Err(_) => {
                error!("could not read subdir");
                return;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            return;
        }
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                error!("could not read metadata");
                return;
            }
        };
        if let Ok(mod_time) = metadata.modified() {
            let current_time = SystemTime::now();
            if let Ok(age) = current_time.duration_since(mod_time) {
                if age >= config.max_age {
                    debug!(
                        "{:?} - {:?} - {:?}",
                        path.display(),
                        age.as_secs(),
                        config.max_age
                    );
                    let mut job = Job::new("");
                    match job.retrieve_by_loc(path.display().to_string(), &pool).await {
                        Ok(_) => {
                            let _ = job.update_status(Status::Cleaned, &pool).await;
                            if let Err(e) = job.remove_from_disk() {
                                error!("error: {:?} - could not remove {:?}", e, path)
                            }
                        }
                        Err(e) => error!("{:?} - not found: {:?}", e, path),
                    }
                }
            }
        };
    });

    futures::future::join_all(futures).await;
}

pub async fn sender(pool: SqlitePool, config: Config) {
    let mut queue = Queue::new(&config);
    if queue.load(&pool).await.is_ok() {
        // info!("There are {:?} queued jobs", queue.jobs.len());
        let futures = queue
            .jobs
            .into_iter()
            .map(|mut j| {
                // info!("{:?}", j);
                let pool_clone = pool.clone();
                let config_clone = config.clone();
                tokio::spawn(async move {
                    j.update_status(Status::Processing, &pool_clone).await.ok();

                    match orchestrator::send(&j, &config_clone, Client).await {
                        Ok(upload_id) => {
                            info!("submitting: {:?}", j);
                            j.update_status(Status::Submitted, &pool_clone).await.ok();
                            j.update_dest_id(upload_id, &pool_clone).await.ok();
                            debug!("{:?}", j);
                        }
                        Err(e) => {
                            error!("Upload error: {:?}", e);
                            j.update_status(Status::Failed, &pool_clone).await.ok();
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        futures::future::join_all(futures).await;
    }
}

pub async fn getter(pool: SqlitePool, config: Config) {
    let mut queue = Queue::new(&config);

    if let Err(e) = queue.list_per_status(Status::Submitted, &pool).await {
        error!("Failed to fetch submitted jobs: {:?}", e);
        return;
    }

    let _: Vec<_> = stream::iter(queue.jobs)
        .map(|mut j| {
            let pool = pool.clone();
            let config = config.clone();
            async move {
                match orchestrator::retrieve(&j, &config, Client).await {
                    Ok(_) => {
                        if let Err(e) = j.update_status(Status::Completed, &pool).await {
                            error!("Failed to update job {} to Completed: {:?}", j.id, e);
                        } else {
                            info!("Job {} completed successfully", j.id);
                        }
                    }
                    Err(DownloadError::JobNotReady) => {
                        debug!("Job {} not ready yet", j.id);
                    }
                    Err(DownloadError::JobNotFound) => {
                        warn!("Job {} not found on server", j.id);
                        j.update_status(Status::Unknown, &pool).await.ok();
                    }
                    Err(e) => {
                        error!("Failed to download job {}: {:?}", j.id, e);
                        j.update_status(Status::Unknown, &pool).await.ok();
                    }
                }
            }
        })
        // NOTE: This will limit how many "retrieves" we are doing at a single time, this might
        // be relevant to avoid overloading the system
        .buffer_unordered(10)
        .collect()
        .await;
}

// Client side
pub async fn runner(pool: SqlitePool, config: Config) {
    let mut queue = PayloadQueue::new(&config);
    if queue.list_per_status(Status::Prepared, &pool).await.is_ok() {
        let futures = queue
            .jobs
            .into_iter()
            .map(|mut j| {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    match execute_payload(&j) {
                        Ok(_) => {
                            j.update_status(Status::Completed, &pool_clone).await.ok();
                        }
                        // TODO: Add specific error handler for each case
                        Err(
                            ClientError::Execution
                            | ClientError::Script
                            | ClientError::NoExecScript,
                        ) => {
                            j.update_status(Status::Failed, &pool_clone).await.ok();
                        }
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
    use crate::config::loader::{Config, Service};
    use crate::models::payload_dao::Payload;
    use crate::models::{job_dao::Job, job_dto::create_jobs_table};
    use std::{path::Path, time::Duration};
    use tempfile::TempDir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_sender() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.services.insert(
            "A".to_string(),
            Service {
                name: "A".to_string(),
                upload_url: "http://example.com/upload_a".to_string(),
                download_url: "http://example.com/download_a".to_string(),
                runs_per_user: 5,
            },
        );

        create_jobs_table(&pool).await.unwrap();

        // add a job
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_service("A".to_string());
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Queued, &pool).await.unwrap();
        let id = job.id;

        sender(pool.clone(), config).await;

        let tempdir = TempDir::new().unwrap();
        let mut _job = Job::new(tempdir.path().to_str().unwrap());
        _job.retrieve_id(id, &pool).await.unwrap();

        // Since nothing is configured, it will fail
        //  the only thing we need to test here is if
        //  the status is being updated
        assert_eq!(_job.status, Status::Failed);

        // TODO: Add mock the `send` function to test the match arm
    }

    #[tokio::test]
    async fn test_getter() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let config = Config::new().unwrap();

        create_jobs_table(&pool).await.unwrap();

        // add a job
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Submitted, &pool).await.unwrap();
        let id = job.id;

        getter(pool.clone(), config).await;

        let tempdir = TempDir::new().unwrap();
        let mut _job = Job::new(tempdir.path().to_str().unwrap());
        _job.retrieve_id(id, &pool).await.unwrap();

        // Since nothing is configured, it will fail
        //  and set the job as Unknown
        assert_eq!(_job.status, Status::Unknown);

        // TODO: Add mock the `retrieve` function to test the match arm
    }

    #[tokio::test]
    async fn test_cleaner() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();

        create_jobs_table(&pool).await.unwrap();

        // add a job
        let tempdir = TempDir::new().unwrap();
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        fs::create_dir_all(&job.loc).unwrap();

        job.add_to_db(&pool).await.unwrap();

        config.max_age = Duration::from_nanos(1);
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Sleep to allow the file to age
        // NOTE: This is simpler than editing the mtime
        sleep(Duration::from_nanos(1)).await;

        assert!(Path::new(&job.loc).exists());

        cleaner(pool.clone(), config).await;

        assert!(!Path::new(&job.loc).exists());

        let mut _job = Job::new("");
        let _ = _job.retrieve_id(job.id, &pool).await;

        assert_eq!(_job.status, Status::Cleaned);
    }

    #[tokio::test]
    async fn test_runner() {
        // Initialize pool
        let pool = crate::datasource::db::init_payload_db().await;
        // Initialize config
        let mut config = Config::new().unwrap();
        let tempdir = TempDir::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();

        // Add a payload
        let mut payload = Payload::new();
        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        // Add input data
        let data = b"#!/bin/bash\necho 'Hello, World!' > output.txt\n";
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

        // Run the runner
        runner(pool.clone(), config).await;

        // Check the effects
        let mut _payload = Payload::retrieve_id(payload.id, &pool)
            .await
            .expect("Failed to retrieve payload");

        assert_eq!(_payload.status, Status::Completed);
        let expected_output = tempdir
            .path()
            .join(payload.id.to_string())
            .join("output.txt");
        assert!(expected_output.exists());
    }

    #[tokio::test]
    async fn test_runner_failure() {
        // Initialize pool
        let pool = crate::datasource::db::init_payload_db().await;
        // Initialize config
        let config = Config::new().unwrap();

        // Add a payload
        let mut payload = Payload::new();
        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        // Add input data
        let data = b"#!/bin/bash\nexit 1\n";
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

        // Run the runner
        runner(pool.clone(), Config::new().unwrap()).await;

        // Check the effects
        // NOTE: You need to retrieve the payload again to get the updated status
        let mut _payload = Payload::retrieve_id(payload.id, &pool)
            .await
            .expect("Failed to retrieve payload");

        assert_eq!(_payload.status, Status::Failed);
    }
}
