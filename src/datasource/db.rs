use crate::models::job_dto::create_jobs_table;
use crate::models::payload_dto::create_payload_table;
use sqlx::{Pool, Sqlite, SqlitePool};
use tracing::info;

pub async fn init_db(db_path: &str) -> Pool<Sqlite> {
    let connection_string = format!("sqlite://{db_path}?mode=rwc").to_string();
    info!("Using database: {}", connection_string);
    let pool = SqlitePool::connect(&connection_string)
        .await
        .unwrap_or_else(|e| panic!("Database connection failed: {e}"));

    create_jobs_table(&pool)
        .await
        .expect("failed to create the jobs table");

    pool
}

pub async fn init_payload_db() -> Pool<Sqlite> {
    let connection_string = "sqlite::memory:".to_string();
    info!("Using in-memory database");
    let pool = SqlitePool::connect(&connection_string)
        .await
        .unwrap_or_else(|e| panic!("Database connection failed: {e}"));

    create_payload_table(&pool)
        .await
        .expect("failed to create the payloads table");

    pool
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_db_success() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let pool = init_db(db_path_str).await;

        // Verify connection is valid
        assert!(!pool.is_closed());

        // Verify jobs table was created by querying it
        let result = sqlx::query("SELECT COUNT(*) FROM jobs")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_db_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_create.db");
        let db_path_str = db_path.to_str().unwrap();

        assert!(!db_path.exists());

        let pool = init_db(db_path_str).await;

        // Database file should be created
        assert!(db_path.exists());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_db_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_idempotent.db");
        let db_path_str = db_path.to_str().unwrap();

        // Initialize once
        let pool1 = init_db(db_path_str).await;
        pool1.close().await;

        // Initialize again - should not fail
        let pool2 = init_db(db_path_str).await;
        assert!(!pool2.is_closed());

        pool2.close().await;
    }

    #[tokio::test]
    async fn test_init_payload_db_success() {
        let pool = init_payload_db().await;

        // Verify connection is valid
        assert!(!pool.is_closed());

        // Verify payloads table was created by querying it
        let result = sqlx::query("SELECT COUNT(*) FROM payloads")
            .fetch_one(&pool)
            .await;
        assert!(result.is_ok());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_payload_db_in_memory() {
        let pool = init_payload_db().await;

        // Insert a test record
        let insert_result = sqlx::query(
            "INSERT INTO payloads (id, status, loc) VALUES (1, 'Pending', '/test/path')",
        )
        .execute(&pool)
        .await;
        assert!(insert_result.is_ok());

        // Query it back
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payloads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_payload_db_multiple_instances() {
        // Each in-memory database should be independent
        let pool1 = init_payload_db().await;
        let pool2 = init_payload_db().await;

        // Insert into pool1
        sqlx::query("INSERT INTO payloads (id, status, loc) VALUES (1, 'Pending', '/test/path')")
            .execute(&pool1)
            .await
            .unwrap();

        // pool2 should be empty
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payloads")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(count.0, 0);

        pool1.close().await;
        pool2.close().await;
    }
}
