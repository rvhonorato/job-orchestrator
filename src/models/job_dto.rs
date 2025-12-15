use std::path::PathBuf;

use crate::models::job_dao::Job;
use crate::models::status_dto::Status;
use sqlx::{Row, SqlitePool};

pub async fn create_jobs_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            service TEXT NOT NULL,
            status TEXT NOT NULL,
            loc TEXT NOT NULL,
            dest_id INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

impl Job {
    pub async fn add_to_db(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let result =
            sqlx::query("INSERT INTO jobs (user_id, loc, status, service) VALUES (?, ?, ?, ?)")
                .bind(self.user_id)
                .bind(self.loc.to_str())
                .bind(self.status.to_string())
                .bind(self.service.to_string())
                .execute(pool)
                .await?;

        let job_id = result.last_insert_rowid();
        self.id = job_id as i32;

        Ok(())
    }

    pub async fn update_status(
        &mut self,
        status: Status,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let _result = sqlx::query("UPDATE jobs SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(self.id)
            .execute(pool)
            .await?;

        self.status = status;

        Ok(())
    }

    pub async fn update_dest_id(
        &mut self,
        dest_id: u32,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let _result = sqlx::query("UPDATE jobs SET dest_id = ? WHERE id = ?")
            .bind(dest_id)
            .bind(self.id)
            .execute(pool)
            .await?;

        self.dest_id = dest_id;

        Ok(())
    }

    pub async fn retrieve_id(&mut self, id: i32, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;

        let status: String = row.get("status");
        let loc: String = row.get("loc");

        self.id = row.get("id");
        self.user_id = row.get("user_id");
        self.service = row.get("service");
        self.status = Status::from_string(&status);
        self.loc = PathBuf::from(loc);
        self.dest_id = row.get("dest_id");

        Ok(())
    }

    pub async fn retrieve_by_loc(
        &mut self,
        loc: String,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let row = sqlx::query("SELECT * FROM jobs WHERE loc = ?")
            .bind(loc)
            .fetch_optional(pool)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;

        let status: String = row.get("status");
        let loc: String = row.get("loc");

        self.id = row.get("id");
        self.user_id = row.get("user_id");
        self.service = row.get("service");
        self.status = Status::from_string(&status);
        self.loc = PathBuf::from(loc);
        self.dest_id = row.get("dest_id");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::job_dao::Job;
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        create_jobs_table(&pool).await.unwrap();
        pool
    }

    // ===== create_jobs_table tests =====

    #[tokio::test]
    async fn test_create_jobs_table() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let result = create_jobs_table(&pool).await;
        assert!(result.is_ok());

        // Verify table exists by querying it
        let query_result =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='jobs'")
                .fetch_one(&pool)
                .await;
        assert!(query_result.is_ok());
    }

    #[tokio::test]
    async fn test_create_jobs_table_idempotent() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Create table twice
        let result1 = create_jobs_table(&pool).await;
        let result2 = create_jobs_table(&pool).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    // ===== add_to_db tests =====

    #[tokio::test]
    async fn test_add_to_db() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(42);
        job.set_service("test_service".to_string());

        let result = job.add_to_db(&pool).await;
        assert!(result.is_ok());
        assert!(job.id > 0); // ID should be assigned
    }

    #[tokio::test]
    async fn test_add_to_db_assigns_incremental_ids() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job1 = Job::new(tempdir.path().to_str().unwrap());
        job1.set_user_id(1);
        job1.set_service("service1".to_string());
        job1.add_to_db(&pool).await.unwrap();

        let mut job2 = Job::new(tempdir.path().to_str().unwrap());
        job2.set_user_id(2);
        job2.set_service("service2".to_string());
        job2.add_to_db(&pool).await.unwrap();

        assert_eq!(job1.id, 1);
        assert_eq!(job2.id, 2);
    }

    // ===== update_status tests =====

    #[tokio::test]
    async fn test_update_status() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();

        // Update status
        let result = job.update_status(Status::Processing, &pool).await;
        assert!(result.is_ok());
        assert_eq!(job.status, Status::Processing);

        // Verify in database by retrieving and checking the deserialized status
        let row = sqlx::query("SELECT status FROM jobs WHERE id = ?")
            .bind(job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let status_str: String = row.get("status");
        let status = Status::from_string(&status_str);
        assert_eq!(status, Status::Processing);
    }

    #[tokio::test]
    async fn test_update_status_multiple_transitions() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();

        // Test multiple status transitions
        job.update_status(Status::Queued, &pool).await.unwrap();
        assert_eq!(job.status, Status::Queued);

        job.update_status(Status::Processing, &pool).await.unwrap();
        assert_eq!(job.status, Status::Processing);

        job.update_status(Status::Completed, &pool).await.unwrap();
        assert_eq!(job.status, Status::Completed);
    }

    // ===== update_dest_id tests =====

    #[tokio::test]
    async fn test_update_dest_id() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();

        // Update dest_id
        let result = job.update_dest_id(123, &pool).await;
        assert!(result.is_ok());
        assert_eq!(job.dest_id, 123);

        // Verify in database
        let row = sqlx::query("SELECT dest_id FROM jobs WHERE id = ?")
            .bind(job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let dest_id: u32 = row.get("dest_id");
        assert_eq!(dest_id, 123);
    }

    #[tokio::test]
    async fn test_update_dest_id_can_change() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();

        job.update_dest_id(100, &pool).await.unwrap();
        assert_eq!(job.dest_id, 100);

        job.update_dest_id(200, &pool).await.unwrap();
        assert_eq!(job.dest_id, 200);
    }

    // ===== retrieve_id tests =====

    #[tokio::test]
    async fn test_retrieve_id() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        // Create and save a job
        let mut original_job = Job::new(tempdir.path().to_str().unwrap());
        original_job.set_user_id(42);
        original_job.set_service("test_service".to_string());
        original_job.add_to_db(&pool).await.unwrap();
        original_job
            .update_status(Status::Processing, &pool)
            .await
            .unwrap();
        original_job.update_dest_id(999, &pool).await.unwrap();

        // Retrieve it
        let mut retrieved_job = Job::new("");
        let result = retrieved_job.retrieve_id(original_job.id, &pool).await;

        assert!(result.is_ok());
        assert_eq!(retrieved_job.id, original_job.id);
        assert_eq!(retrieved_job.user_id, original_job.user_id);
        assert_eq!(retrieved_job.service, original_job.service);
        assert_eq!(retrieved_job.status, original_job.status);
        assert_eq!(retrieved_job.loc, original_job.loc);
        assert_eq!(retrieved_job.dest_id, original_job.dest_id);
    }

    #[tokio::test]
    async fn test_retrieve_id_not_found() {
        let pool = setup_test_db().await;

        let mut job = Job::new("");
        let result = job.retrieve_id(999, &pool).await;

        assert!(result.is_err());
        match result {
            Err(sqlx::Error::RowNotFound) => (),
            _ => panic!("Expected RowNotFound error"),
        }
    }

    // ===== retrieve_by_loc tests =====

    #[tokio::test]
    async fn test_retrieve_by_loc() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        // Create and save a job
        let mut original_job = Job::new(tempdir.path().to_str().unwrap());
        original_job.set_user_id(42);
        original_job.set_service("test_service".to_string());
        original_job.add_to_db(&pool).await.unwrap();
        original_job
            .update_status(Status::Completed, &pool)
            .await
            .unwrap();

        let loc_str = original_job.loc.to_str().unwrap().to_string();

        // Retrieve by location
        let mut retrieved_job = Job::new("");
        let result = retrieved_job.retrieve_by_loc(loc_str, &pool).await;

        assert!(result.is_ok());
        assert_eq!(retrieved_job.id, original_job.id);
        assert_eq!(retrieved_job.user_id, original_job.user_id);
        assert_eq!(retrieved_job.service, original_job.service);
        assert_eq!(retrieved_job.status, original_job.status);
        assert_eq!(retrieved_job.loc, original_job.loc);
    }

    #[tokio::test]
    async fn test_retrieve_by_loc_not_found() {
        let pool = setup_test_db().await;

        let mut job = Job::new("");
        let result = job
            .retrieve_by_loc("/nonexistent/path".to_string(), &pool)
            .await;

        assert!(result.is_err());
        match result {
            Err(sqlx::Error::RowNotFound) => (),
            _ => panic!("Expected RowNotFound error"),
        }
    }

    // ===== Integration tests =====

    #[tokio::test]
    async fn test_full_job_lifecycle() {
        let pool = setup_test_db().await;
        let tempdir = TempDir::new().unwrap();

        // Create job
        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("integration_test".to_string());

        // Add to database
        job.add_to_db(&pool).await.unwrap();
        let job_id = job.id;

        // Update through various statuses
        job.update_status(Status::Queued, &pool).await.unwrap();
        job.update_status(Status::Processing, &pool).await.unwrap();
        job.update_dest_id(555, &pool).await.unwrap();
        job.update_status(Status::Submitted, &pool).await.unwrap();
        job.update_status(Status::Completed, &pool).await.unwrap();

        // Retrieve and verify
        let mut retrieved = Job::new("");
        retrieved.retrieve_id(job_id, &pool).await.unwrap();

        assert_eq!(retrieved.status, Status::Completed);
        assert_eq!(retrieved.dest_id, 555);
        assert_eq!(retrieved.user_id, 1);
        assert_eq!(retrieved.service, "integration_test");
    }
}
