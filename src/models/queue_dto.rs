use std::path::{Path, PathBuf};

use super::{queue_dao::Queue, status_dto::Status};
use crate::models::{job_dao::Job, payload_dao::Payload, queue_dao::PayloadQueue};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

impl Queue<'_> {
    pub async fn list_per_status(
        &mut self,
        status: Status,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = ?")
            .bind(status.to_string())
            .fetch_all(pool)
            .await?;

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
        // Step 1: Get all QUEUED jobs
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = ?")
            .bind(Status::Queued.to_string())
            .fetch_all(pool)
            .await?;

        // ===========================================================================================
        // Step 2: Get submitted job counts per user/service
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

        // ===========================================================================================
        // Step 3: Filter jobs according to config limits
        // jobs_by_user_service will hold the jobs to be processed
        let mut jobs_by_user_service: HashMap<(i64, String), Vec<Job>> = HashMap::new();
        // service_limits will cache the limits per service, so we don't have to look them up
        // multiple times
        let mut service_limits: HashMap<String, u16> = HashMap::new();
        for row in rows {
            let user_id: i64 = row.get("user_id");
            let service: String = row.get("service");
            // let status: String = row.get("status");
            // info!(
            //     "DB Row: user_id={}, service={}, status={}",
            //     user_id, service, status
            // );

            // Check what is the limit for this service
            let limit = *service_limits.entry(service.clone()).or_insert_with(|| {
                self.config
                    .services
                    .get(&service)
                    .map(|s| s.runs_per_user)
                    .unwrap()
            });
            // let limit = 5;
            let submitted = *submitted_counts
                .get(&(user_id, service.clone()))
                .unwrap_or(&0);
            // info!(
            //     "User: {}, Service: {}, Submitted: {}, Limit: {}",
            //     user_id, service, submitted, limit
            // );
            // Check if this user/service combo can take more jobs
            let key = (user_id, service.clone());
            let user_queue = jobs_by_user_service.entry(key).or_default();
            let remaining_slots = (limit - submitted) as usize;
            // if submitted < limit, we can add more jobs, it has not yet reached the limit
            // if user_queue.len() < remaining_slots, we can still add to this user's queue
            // info!(
            //     "User: {}, Service: {}, Current Queue Length: {}, Remaining Slots: {}",
            //     user_id,
            //     service,
            //     user_queue.len(),
            //     remaining_slots
            // );
            if submitted < limit && user_queue.len() < remaining_slots {
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
            }
        }

        // ===========================================================================================
        // Step 4: Flatten the jobs_by_user_service into self.jobs
        self.jobs = jobs_by_user_service.into_values().flatten().collect();
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
                runs_per_user: 5,
            },
        );
        config.services.insert(
            "B".to_string(),
            Service {
                name: "B".to_string(),
                upload_url: "http://example.com/upload_b".to_string(),
                download_url: "http://example.com/download_b".to_string(),
                runs_per_user: 5,
            },
        );
        config.services.insert(
            "C".to_string(),
            Service {
                name: "C".to_string(),
                upload_url: "http://example.com/upload_c".to_string(),
                download_url: "http://example.com/download_c".to_string(),
                runs_per_user: 1,
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

        // Add jobs for service C, which has a limit of 1
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

        // Since the limit for service C per user is 1, there should be two jobs loaded in total
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
