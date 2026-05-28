use std::path::{Path, PathBuf};

use super::{queue_dao::Queue, status_dto::Status};
use crate::models::{job_dao::Job, payload_dao::Payload, queue_dao::PayloadQueue};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

impl Queue<'_> {
    pub async fn list_per_status(
        &mut self,
        statuses: Vec<Status>,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let mut qb = sqlx::QueryBuilder::new("SELECT * FROM jobs WHERE status IN (");
        let mut sep = qb.separated(", ");
        for s in &statuses {
            sep.push_bind(s.to_string());
        }
        qb.push(")");

        let rows = qb.build().fetch_all(pool).await?;

        let jobs: Vec<Job> = rows
            .into_iter()
            .map(|row| {
                let status: String = row.get("status");
                let loc: String = row.get("loc");
                let dest_id: u32 = row.get("dest_id");
                Job {
                    id: row.get("id"),
                    user_id: row.get("user_id"),
                    service: row.get("service"),
                    status: Status::from_string(&status),
                    loc: PathBuf::from(loc),
                    dest_id,
                }
            })
            .collect();
        self.jobs = jobs;
        Ok(())
    }

    pub async fn load(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // ===========================================================================================
        // Step 1a: get how many jobs have been submitted to the service
        let submitted_rows = sqlx::query(
            "SELECT user_id, service, COUNT(*) as count FROM jobs WHERE status = 'submitted' GROUP BY user_id, service"
        )
        .fetch_all(pool)
        .await?;
        let mut submitted_counts: HashMap<(i64, String), u16> = HashMap::new();
        for row in submitted_rows {
            let user_id: i64 = row.get("user_id");
            let service: String = row.get("service");
            let count: i64 = row.get("count");
            submitted_counts.insert((user_id, service), count as u16);
        }

        // Step 1b: get submitted job counts per service (for max_runs limit)
        let submitted_service_rows = sqlx::query(
            "SELECT service, COUNT(*) as count FROM jobs WHERE status = 'submitted' GROUP BY service"
        )
        .fetch_all(pool)
        .await?;
        let mut submitted_service_counts: HashMap<String, u16> = HashMap::new();
        for row in submitted_service_rows {
            let service: String = row.get("service");
            let count: i64 = row.get("count");
            submitted_service_counts.insert(service, count as u16);
        }

        // ===========================================================================================
        // Step 2: Filter jobs according to config limits

        // jobs_by_user_service will hold the jobs to be processed
        let mut jobs_by_user_service: HashMap<(i64, String), Vec<Job>> = HashMap::new();

        // service_limits will cache the limits per service, so we don't have to look them up
        // multiple times
        let mut service_limits: HashMap<String, u16> = HashMap::new();

        // service_max_runs will cache the max_runs per service
        let mut service_max_runs: HashMap<String, u16> = HashMap::new();

        // service_queued_counts will track how many jobs we're queuing per service in this batch
        let mut service_queued_counts: HashMap<String, usize> = HashMap::new();

        // Get all the QUEUED jobs, these are the ones waiting to be sent
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = ?")
            .bind(Status::Queued.to_string())
            .fetch_all(pool)
            .await?;

        for row in rows {
            let user_id: i64 = row.get("user_id");
            let service: String = row.get("service");

            // Check what is the limit per user for this service
            let quota_per_user = *service_limits.entry(service.clone()).or_insert_with(|| {
                self.config
                    .services
                    .get(&service)
                    .map(|s| s.runs_per_user)
                    .unwrap()
            });

            // Check what is the max limit for this service
            let quota_total = *service_max_runs.entry(service.clone()).or_insert_with(|| {
                self.config
                    .services
                    .get(&service)
                    .map(|s| s.max_runs)
                    .unwrap()
            });

            let user_submitted = *submitted_counts
                .get(&(user_id, service.clone()))
                .unwrap_or(&0);

            // Get total submitted count for this service
            let service_submitted = *submitted_service_counts.get(&service).unwrap_or(&0);

            // Track how many jobs we're queuing for this service in this batch
            let queued_for_service = service_queued_counts.entry(service.clone()).or_default();

            // Check if this user/service combo can take more jobs
            let key = (user_id, service.clone());
            let user_queue = jobs_by_user_service.entry(key).or_default();
            let user_remaining_slots = quota_per_user.saturating_sub(user_submitted) as usize;

            // Check if adding this job would exceed `max_runs` for the service
            // submitted_service + *queued_for_service + 1 (this job) <= max_runs
            let would_exceed_max_runs = {
                let total_if_added = service_submitted as usize + *queued_for_service + 1;
                total_if_added > quota_total as usize
            };

            // Only add job if:
            // 1. User hasn't reached their per-user limit
            // 2. Service hasn't reached its `max_runs` limit (including this job)
            if user_submitted < quota_per_user
                && user_queue.len() < user_remaining_slots
                && !would_exceed_max_runs
            {
                let status: String = row.get("status");
                let loc: String = row.get("loc");
                user_queue.push(Job {
                    id: row.get("id"),
                    user_id: user_id.try_into().unwrap(),
                    service: service.clone(),
                    status: Status::from_string(&status),
                    loc: PathBuf::from(loc),
                    dest_id: row.get("dest_id"),
                });

                // Increment the count for this service
                *queued_for_service += 1;
            }
        }

        // ===========================================================================================
        // Step 4: Flatten the jobs_by_user_service into self.jobs
        self.jobs = jobs_by_user_service.into_values().flatten().collect();

        // Done
        Ok(())
    }
}

impl PayloadQueue<'_> {
    pub async fn list_per_status(
        &mut self,
        status: Status,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT * FROM payloads WHERE status = ?")
            .bind(status.to_string())
            .fetch_all(pool)
            .await?;

        let jobs: Vec<Payload> = rows
            .into_iter()
            .map(|row| {
                let status: String = row.get("status");
                let id: u32 = row.get("id");
                let loc: Option<String> = row.get("loc");

                let mut payload = Payload::new();
                payload.set_id(id);
                payload.set_status(Status::from_string(&status));
                payload.pid = row.get("pid");
                payload.killed = row.get("killed");
                // Use loc from database, or fall back to constructed path for backwards compatibility
                let loc_path = loc
                    .map(PathBuf::from)
                    .unwrap_or_else(|| Path::new(&self.config.data_path).join(id.to_string()));
                payload.set_loc(loc_path);

                payload
            })
            .collect();
        self.jobs = jobs;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::{Config, Service};
    use crate::models::job_dto::create_jobs_table;
    use crate::models::payload_dto::create_payload_table;

    #[tokio::test]
    async fn test_list_per_status_jobs() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.services.insert(
            "svc".to_string(),
            Service {
                name: "svc".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 5,
                max_runs: 1,
            },
        );

        create_jobs_table(&pool).await.unwrap();

        sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'svc', 'submitted', '/tmp/a', NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (2, 'svc', 'running', '/tmp/b', 5)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (3, 'svc', 'queued', '/tmp/c', NULL)")
            .execute(&pool).await.unwrap();

        let mut queue = Queue::new(&config);
        queue
            .list_per_status(vec![Status::Submitted, Status::Running], &pool)
            .await
            .unwrap();

        assert_eq!(queue.jobs.len(), 2);
        assert!(queue.jobs.iter().any(|j| j.status == Status::Submitted));
        assert!(queue.jobs.iter().any(|j| j.status == Status::Running));
        assert!(queue.jobs.iter().all(|j| j.status != Status::Queued));
    }

    #[tokio::test]
    async fn test_load_limits_jobs_per_user_per_service() {
        // Setup in-memory SQLite database
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
                max_runs: 10, // High enough to allow multiple users
            },
        );
        config.services.insert(
            "B".to_string(),
            Service {
                name: "B".to_string(),
                upload_url: "http://example.com/upload_b".to_string(),
                download_url: "http://example.com/download_b".to_string(),
                terminate_url: "http://example.com/terminate_b".to_string(),
                runs_per_user: 5,
                max_runs: 10, // High enough to allow multiple users
            },
        );
        config.services.insert(
            "C".to_string(),
            Service {
                name: "C".to_string(),
                upload_url: "http://example.com/upload_c".to_string(),
                download_url: "http://example.com/download_c".to_string(),
                terminate_url: "http://example.com/terminate_c".to_string(),
                runs_per_user: 1,
                max_runs: 3, // Allow 3 total: 2 for users 1+2, 1 for user 3
            },
        );

        create_jobs_table(&pool).await.unwrap();

        // Insert 5 submitted jobs for user 1 - service A
        for _ in 0..5 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'A', 'submitted', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }
        // Insert 2 queued jobs for user 1 - service A
        for _ in 0..2 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'A', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        // Insert 3 submitted jobs for user 1 - service B
        for _ in 0..3 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'B', 'submitted', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }
        // Insert 3 queued jobs for user 1 - service B
        for _ in 0..3 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'B', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        // Here user 1 has:
        //  - 5 submitted / 2 queued jobs for service A
        //  - 3 submitted / 3 queued jobs for service B

        // Load the queue
        let mut queue = Queue::new(&config);
        queue.load(&pool).await.unwrap();

        // User already has 5 submitted for service A,
        // > no more queued jobs for service A should be loaded
        let jobs_for_a = queue.jobs.iter().filter(|j| j.service == "A").count();
        let expected_a = 0;
        assert_eq!(jobs_for_a, expected_a,);
        // User has 3 submitted and 3 queued for service B,
        // > 2 more queued jobs for service B should be loaded, since max is 5
        let jobs_for_b = queue.jobs.iter().filter(|j| j.service == "B").count();
        let expected_b = 2;
        assert_eq!(jobs_for_b, expected_b,);

        // Add more jobs for another user
        for _ in 0..2 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (2, 'A', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        // Reload the queue
        queue.load(&pool).await.unwrap();

        // Now there should be 2 jobs for user 2 - service A
        let jobs_for_a = queue.jobs.iter().filter(|j| j.service == "A").count();
        let expected_a = 2;
        assert_eq!(jobs_for_a, expected_a,);

        // Add jobs for service C, which has a limit of 1 per user and max_runs of 3
        // Add two queued jobs for user 1 - service C
        for _ in 0..2 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'C', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }
        // Add two queued jobs for user 2 - service C
        for _ in 0..2 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (2, 'C', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }
        // Reload the queue
        queue.load(&pool).await.unwrap();

        // Since the limit for service C per user is 1 and max_runs is 3,
        // there should be two jobs loaded (one per user 1 and 2)
        let jobs_for_c = queue.jobs.iter().filter(|j| j.service == "C").count();
        let expected_c = 2;
        assert_eq!(jobs_for_c, expected_c);

        // Add more jobs for user 3 to test isolation
        for _ in 0..3 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (3, 'A', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        for _ in 0..4 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (3, 'B', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        for _ in 0..2 {
            sqlx::query("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (3, 'C', 'queued', 'loc', NULL)")
                .execute(&pool).await.unwrap();
        }

        // Reload the queue
        queue.load(&pool).await.unwrap();
        let jobs_for_user3: Vec<&Job> = queue.jobs.iter().filter(|j| j.user_id == 3).collect();
        let expected_user3 = 8; // 3 (A) + 4 (B) + 1 (C)
        assert_eq!(jobs_for_user3.len(), expected_user3);
    }

    #[tokio::test]
    async fn test_load_respects_max_runs_per_service() {
        // Test that max_runs limits total concurrent jobs per service
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.services.insert(
            "test_service".to_string(),
            Service {
                name: "test_service".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 10, // High per-user limit
                max_runs: 2,       // But only 2 total concurrent per service
            },
        );

        create_jobs_table(&pool).await.unwrap();

        // Insert 5 submitted jobs for service "test_service" (across different users)
        for user_id in 1..=5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES ({}, 'test_service', 'submitted', 'loc', NULL)", user_id))
                .execute(&pool).await.unwrap();
        }

        // Insert 5 queued jobs for service "test_service" (across different users)
        for user_id in 1..=5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES ({}, 'test_service', 'queued', 'loc', NULL)", user_id))
                .execute(&pool).await.unwrap();
        }

        // Load the queue
        let mut queue = Queue::new(&config);
        queue.load(&pool).await.unwrap();

        // Since max_runs is 2 and there are already 5 submitted,
        // no queued jobs should be loaded (service has reached max_runs)
        let jobs_loaded = queue.jobs.len();
        assert_eq!(
            jobs_loaded, 0,
            "No jobs should be loaded when max_runs is exceeded for the service"
        );

        // Now test with available slots: remove submitted jobs to create space
        sqlx::query("DELETE FROM jobs WHERE status = 'submitted'")
            .execute(&pool)
            .await
            .unwrap();

        // Reload the queue
        queue.load(&pool).await.unwrap();

        // Now with 0 submitted, we should be able to load up to max_runs (2) queued jobs
        let jobs_loaded = queue.jobs.len();
        assert_eq!(jobs_loaded, 2, "Should load up to max_runs (2) jobs");
    }

    #[tokio::test]
    async fn test_list_per_status_payloads() {
        // Setup in-memory SQLite database
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.data_path = "./data".to_string();

        // Create payloads table
        let _ = create_payload_table(&pool).await;

        // Insert payloads with different statuses
        sqlx::query("INSERT INTO payloads (status) VALUES ('prepared')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO payloads (status) VALUES ('processing')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO payloads (status) VALUES ('prepared')")
            .execute(&pool)
            .await
            .unwrap();

        // Load queued payloads
        let mut payload_queue = PayloadQueue::new(&config);
        payload_queue
            .list_per_status(Status::Prepared, &pool)
            .await
            .unwrap();

        // There should be 2 queued payloads
        let queued_count = payload_queue.jobs.len();
        let expected_count = 2;
        assert_eq!(queued_count, expected_count);
    }
}
