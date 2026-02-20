use std::fs;
use std::time::SystemTime;

use crate::config::loader::Config;
use crate::models::job_dao::Job;
use crate::models::queue_dao::PayloadQueue;
use crate::models::{queue_dao::Queue, status_dto::Status};
use crate::services::client::{Client, ClientError, execute_payload};
use crate::services::orchestrator;
use futures::stream::{self, StreamExt};
use sqlx::SqlitePool;
use tracing::info;
use tracing::{debug, error};

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

            if let Ok(age) = current_time.duration_since(mod_time)
                && age >= config.max_age
            {
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

// The getter task retrieves the jobs from the Client and updates the status on the Server
pub async fn getter(pool: SqlitePool, config: Config) {
    let mut queue = Queue::new(&config);

    if let Err(e) = queue
        .list_per_status(vec![Status::Submitted, Status::Running], &pool)
        .await
    {
        error!("Failed to fetch submitted jobs: {:?}", e);
        return;
    }

    let _: Vec<_> = stream::iter(queue.jobs)
        .map(|mut j| {
            let pool = pool.clone();
            let config = config.clone();
            async move {
                match orchestrator::retrieve(&j, &config, Client).await {
                    Ok(s) => {
                        if let Err(e) = j.update_status(s, &pool).await {
                            error!("Failed to update status of job {} to {}: {:?}", j.id, s, e);
                        }
                    }
                    Err(e) => {
                        // Log the error but leave the job status unchanged to avoid
                        // incorrectly marking transient conditions (e.g., job still
                        // running) as permanently failed.
                        error!(
                            "There was some error while trying to retrieve job {0} from the client: {e}",
                            j.id
                        );
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
                    // Mark the job as running, without this status it will stay in `Processing`
                    j.update_status(Status::Running, &pool_clone).await.ok();

                    match execute_payload(&j) {
                        Ok(s) => {
                            j.update_status(s, &pool_clone).await.ok();
                        }
                        Err(e) => {
                            error!("There was an error while executing the payload: {e}");
                            let status = match e {
                                ClientError::NoExecScript | ClientError::UnsafeScript { .. } => {
                                    Status::Invalid
                                }
                                ClientError::Execution => Status::Failed,
                            };
                            j.update_status(status, &pool_clone).await.ok();
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
    async fn test_cleaner_invalid_path() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let mut config = Config::new().unwrap();
        config.data_path = "/nonexistent/path/does/not/exist".to_string();

        // Should not panic — cleaner logs the error and returns
        cleaner(pool, config).await;
    }

    #[tokio::test]
    async fn test_cleaner_dir_not_in_db() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let tempdir = TempDir::new().unwrap();
        let orphan_dir = tempdir.path().join("orphan_job");
        fs::create_dir_all(&orphan_dir).unwrap();

        let mut config = Config::new().unwrap();
        config.data_path = tempdir.path().to_str().unwrap().to_string();
        config.max_age = Duration::from_nanos(1);

        sleep(Duration::from_millis(1)).await;

        cleaner(pool, config).await;

        // Directory is still there — retrieve_by_loc failed so nothing was removed
        assert!(orphan_dir.exists());
    }

    #[tokio::test]
    async fn test_sender_success() {
        let tempdir = TempDir::new().unwrap();
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let mut server = mockito::Server::new_async().await;

        // Build a Payload response that Client::upload expects
        let mut mock_payload = Payload::new();
        mock_payload.set_id(42);
        mock_payload.set_status(crate::models::status_dto::Status::Prepared);
        let mock_body = serde_json::to_string(&mock_payload).unwrap();

        let mock = server
            .mock("POST", "/submit")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_body)
            .create_async()
            .await;

        let mut config = Config::new().unwrap();
        config.services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: format!("{}/submit", server.url()),
                download_url: format!("{}/retrieve", server.url()),
                runs_per_user: 5,
                max_runs: u16::MAX,
            },
        );

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_service("test".to_string());
        fs::create_dir_all(&job.loc).unwrap();
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Queued, &pool).await.unwrap();
        let job_id = job.id;

        sender(pool.clone(), config).await;

        mock.assert_async().await;

        let tempdir2 = TempDir::new().unwrap();
        let mut updated = Job::new(tempdir2.path().to_str().unwrap());
        updated.retrieve_id(job_id, &pool).await.unwrap();
        assert_eq!(updated.status, Status::Submitted);
        assert_eq!(updated.dest_id, 42);
    }

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
                max_runs: u16::MAX,
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

    /// When a script exits with non-zero code, the job is still "completed" -
    /// it ran successfully but had a logical failure (e.g., bad user input).
    #[tokio::test]
    async fn test_runner_script_nonzero_exit_is_completed() {
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

        // Add input data - script exits with non-zero code
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
        runner(pool.clone(), config).await;

        // Check the effects
        // NOTE: You need to retrieve the payload again to get the updated status
        let mut _payload = Payload::retrieve_id(payload.id, &pool)
            .await
            .expect("Failed to retrieve payload");

        // Script ran and exited - job is completed (not failed)
        assert_eq!(_payload.status, Status::Completed);
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
}
