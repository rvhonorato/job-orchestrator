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
        // Step 1a: get how many jobs have been submitted to the service per user
        let submitted_rows = sqlx::query(
            "SELECT user_id, service, COUNT(*) as count FROM jobs WHERE status IN ('processing', 'submitted', 'running') GROUP BY user_id, service"
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
            "SELECT service, COUNT(*) as count FROM jobs WHERE status IN ('processing', 'submitted', 'running')  GROUP BY service"
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
        // Step 2: Get all QUEUED jobs and group them by service, then by user
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = ?")
            .bind(Status::Queued.to_string())
            .fetch_all(pool)
            .await?;

        // Group queued jobs: service -> user_id -> Vec<Job>
        let mut service_user_jobs: HashMap<String, HashMap<i64, Vec<Job>>> = HashMap::new();

        for row in rows {
            let user_id: i64 = row.get("user_id");
            let service: String = row.get("service");
            let status: String = row.get("status");
            let loc: String = row.get("loc");
            let job = Job {
                id: row.get("id"),
                user_id: user_id.try_into().unwrap(),
                service: service.clone(),
                status: Status::from_string(&status),
                loc: PathBuf::from(loc),
                dest_id: row.get("dest_id"),
            };

            service_user_jobs
                .entry(service)
                .or_default()
                .entry(user_id)
                .or_default()
                .push(job);
        }

        // ===========================================================================================
        // Step 3: For each service, apply round-robin distribution among users
        for (service, user_jobs_map) in service_user_jobs {
            // Get quotas from config
            let config_service = match self.config.services.get(&service) {
                Some(s) => s,
                None => continue, // Skip services not in config
            };
            let quota_per_user = config_service.runs_per_user;
            let quota_total = config_service.max_runs;

            // Get current counts
            let service_submitted = *submitted_service_counts.get(&service).unwrap_or(&0);
            let available_service_slots =
                (quota_total as usize).saturating_sub(service_submitted as usize);

            if available_service_slots == 0 {
                continue; // Service has no available slots
            }

            // Build list of users with their queued jobs and available slots
            let mut users: Vec<(i64, Vec<Job>, usize)> = user_jobs_map
                .into_iter()
                .map(|(user_id, jobs)| {
                    let user_submitted = submitted_counts
                        .get(&(user_id, service.clone()))
                        .unwrap_or(&0);
                    let available_user_slots =
                        (quota_per_user as usize).saturating_sub(*user_submitted as usize);
                    (user_id, jobs, available_user_slots)
                })
                .filter(|(_, _, slots)| *slots > 0)
                .collect();

            // Round-robin: cycle through users, taking one job at a time
            let mut service_count = 0;
            let mut index = 0;

            while service_count < available_service_slots && !users.is_empty() {
                let user_index = index % users.len();
                let (_, ref mut jobs, ref mut available_slots) = users[user_index];

                if !jobs.is_empty() && *available_slots > 0 {
                    // Take one job from this user
                    let job = jobs.remove(0);
                    self.jobs.push(job);
                    service_count += 1;
                    *available_slots -= 1;
                }

                // Remove user if exhausted (no more jobs or no more slots)
                if jobs.is_empty() || *available_slots == 0 {
                    users.remove(user_index);
                    // Adjust index after removal
                    if index >= users.len() {
                        index = 0;
                    }
                } else {
                    index += 1;
                }
            }
        }

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
    async fn test_load_round_robin_distribution() {
        // Test that round-robin distributes slots fairly among users
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.services.insert(
            "service".to_string(),
            Service {
                name: "service".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 5,
                max_runs: 3, // Only 3 total slots for the service
            },
        );

        create_jobs_table(&pool).await.unwrap();

        // User 1 has 5 queued jobs
        for i in 0..5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'service', 'queued', 'loc{}', NULL)", i))
                .execute(&pool).await.unwrap();
        }
        // User 2 has 5 queued jobs
        for i in 0..5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (2, 'service', 'queued', 'loc{}', NULL)", i))
                .execute(&pool).await.unwrap();
        }
        // User 3 has 5 queued jobs
        for i in 0..5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (3, 'service', 'queued', 'loc{}', NULL)", i))
                .execute(&pool).await.unwrap();
        }

        let mut queue = Queue::new(&config);
        queue.load(&pool).await.unwrap();

        // With round-robin and max_runs=3, we should get 1 job from each of 3 different users
        assert_eq!(queue.jobs.len(), 3, "Should load exactly max_runs jobs");

        let user_ids: Vec<i32> = queue.jobs.iter().map(|j| j.user_id).collect();
        // Each user should have at most 1 job loaded (round-robin with 3 slots, 3 users)
        let mut user_counts: HashMap<i32, usize> = HashMap::new();
        for uid in user_ids {
            *user_counts.entry(uid).or_insert(0) += 1;
        }

        // With round-robin, each of the 3 users should have exactly 1 job
        for (_, count) in user_counts {
            assert_eq!(
                count, 1,
                "Each user should have exactly 1 job in round-robin distribution"
            );
        }
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
        // With round-robin, these should be from 2 different users
        let jobs_loaded = queue.jobs.len();
        assert_eq!(jobs_loaded, 2, "Should load up to max_runs (2) jobs");

        // Verify they are from different users (round-robin)
        let user_ids: Vec<i32> = queue.jobs.iter().map(|j| j.user_id).collect();
        assert_eq!(user_ids.len(), 2);
        assert_ne!(
            user_ids[0], user_ids[1],
            "Round-robin should pick from different users"
        );
    }

    #[tokio::test]
    async fn test_load_respects_user_quota() {
        // Test that per-user quota is still respected
        let pool = SqlitePool::connect(":memory:")
            .await
            .unwrap_or_else(|e| panic!("Database connection failed: {e}"));
        let mut config = Config::new().unwrap();
        config.services.insert(
            "service".to_string(),
            Service {
                name: "service".to_string(),
                upload_url: "http://example.com/upload".to_string(),
                download_url: "http://example.com/download".to_string(),
                terminate_url: "http://example.com/terminate".to_string(),
                runs_per_user: 2, // Each user can have at most 2
                max_runs: 10,     // Service can have up to 10
            },
        );

        create_jobs_table(&pool).await.unwrap();

        // User 1 already has 2 submitted jobs (at their limit)
        for i in 0..2 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'service', 'submitted', 'loc{}', NULL)", i))
                .execute(&pool).await.unwrap();
        }
        // User 1 has 5 queued jobs
        for i in 0..5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (1, 'service', 'queued', 'loc{}', NULL)", i+10))
                .execute(&pool).await.unwrap();
        }
        // User 2 has 5 queued jobs
        for i in 0..5 {
            sqlx::query(&format!("INSERT INTO jobs (user_id, service, status, loc, dest_id) VALUES (2, 'service', 'queued', 'loc{}', NULL)", i+20))
                .execute(&pool).await.unwrap();
        }

        let mut queue = Queue::new(&config);
        queue.load(&pool).await.unwrap();

        // User 1 is at their quota (2 submitted), so only User 2's jobs should be loaded
        let user_ids: Vec<i32> = queue.jobs.iter().map(|j| j.user_id).collect();
        for uid in &user_ids {
            assert_eq!(
                *uid, 2,
                "Only user 2 should have jobs loaded (user 1 is at quota)"
            );
        }
        assert_eq!(
            queue.jobs.len(),
            2,
            "Should load 2 jobs from user 2 (max_runs allows more but user quota is 2)"
        );
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
