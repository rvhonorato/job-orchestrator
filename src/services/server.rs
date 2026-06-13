use std::fs;
use std::time::SystemTime;

use crate::config::loader::Config;
use crate::models::job_dao::Job;
use crate::models::{queue_dao::Queue, status_dto::Status};
use crate::services::client::Client;
use crate::services::endpoint::{self, TerminateError};
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

// Terminate task will send a kill command to the client
pub async fn terminate_job(
    mut j: Job,
    pool: SqlitePool,
    config: Config,
) -> Result<(), TerminateError> {
    let original_status = j.get_status();
    // lock the job so no other thread pick it up
    j.update_status(Status::Locked, &pool).await.ok();

    // If the job has never been dispatched to a client (dest_id == 0),
    // there's nothing running remotely to terminate. Mark it as Killed
    // directly without contacting the client.
    if j.dest_id == 0 {
        j.update_status(Status::Killed, &pool).await.ok();
        return Ok(());
    }

    match endpoint::kill(&j, &config, Client).await {
        Ok(_) => {
            // Job was killed
            j.update_status(Status::Killed, &pool).await.ok();
            Ok(())
        }
        Err(_) => {
            // There was an error, do nothing
            j.update_status(original_status, &pool).await.ok();
            Err(TerminateError::GenericError)
        }
    }
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

                    match endpoint::send(&j, &config_clone, Client).await {
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
        .list_per_status(
            vec![Status::Submitted, Status::Prepared, Status::Running],
            &pool,
        )
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
                match  endpoint::retrieve(&j, &config, Client).await {
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
                terminate_url: format!("{}/terminate", server.url()),
                runs_per_user: 5,
                max_runs: 1,
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
                terminate_url: "http://example.com/terminate_a".to_string(),
                runs_per_user: 5,
                max_runs: 1,
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
    async fn test_terminate_job_success() {
        let tempdir = TempDir::new().unwrap();
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let mut config = Config::new().unwrap();
        config.services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 5,
                max_runs: 1,
            },
        );

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_service("test".to_string());
        job.set_user_id(1);
        job.update_dest_id(42, &pool).await.unwrap();
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Running, &pool).await.unwrap();
        let job_id = job.id;

        // Use mockito to mock the HTTP call
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/terminate/42")
            .with_status(200)
            .create_async()
            .await;

        // Update config to use mock server URL
        if let Some(service) = config.services.get_mut("test") {
            service.terminate_url = format!("{}/terminate", server.url());
        }

        let result = terminate_job(job, pool.clone(), config).await;
        assert!(result.is_ok());

        // Verify the job status was updated to Killed
        let mut updated_job = Job::new("");
        updated_job.retrieve_id(job_id, &pool).await.unwrap();
        assert_eq!(updated_job.status, Status::Killed);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_terminate_job_not_dispatched() {
        let tempdir = TempDir::new().unwrap();
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let mut config = Config::new().unwrap();
        config.services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 5,
                max_runs: 1,
            },
        );

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_service("test".to_string());
        job.set_user_id(1);
        // dest_id is left at its default of 0 — job was never dispatched
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Queued, &pool).await.unwrap();
        let job_id = job.id;
        assert_eq!(job.dest_id, 0);

        // Mock the terminate endpoint — it should NEVER be called since
        // the job was never dispatched to the client.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/terminate/0")
            .with_status(200)
            .expect(0)
            .create_async()
            .await;

        if let Some(service) = config.services.get_mut("test") {
            service.terminate_url = format!("{}/terminate", server.url());
        }

        let result = terminate_job(job, pool.clone(), config).await;
        assert!(result.is_ok());

        // Verify the job status was updated to Killed
        let mut updated_job = Job::new("");
        updated_job.retrieve_id(job_id, &pool).await.unwrap();
        assert_eq!(updated_job.status, Status::Killed);

        // The terminate endpoint must never have been hit
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_terminate_job_failure() {
        let tempdir = TempDir::new().unwrap();
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        create_jobs_table(&pool).await.unwrap();

        let mut config = Config::new().unwrap();
        config.services.insert(
            "test".to_string(),
            Service {
                name: "test".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 5,
                max_runs: 1,
            },
        );

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_service("test".to_string());
        job.set_user_id(1);
        job.update_dest_id(42, &pool).await.unwrap();
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Running, &pool).await.unwrap();
        let job_id = job.id;
        let original_status = job.status;

        // Mock the terminate endpoint to return 500
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/terminate/42")
            .with_status(500)
            .create_async()
            .await;

        // Update config to use mock server URL
        if let Some(service) = config.services.get_mut("test") {
            service.terminate_url = format!("{}/terminate", server.url());
        }

        let result = terminate_job(job, pool.clone(), config).await;
        assert!(result.is_err());

        // Verify the job status was restored to original
        let mut updated_job = Job::new("");
        updated_job.retrieve_id(job_id, &pool).await.unwrap();
        assert_eq!(updated_job.status, original_status);

        mock.assert_async().await;
    }
}
