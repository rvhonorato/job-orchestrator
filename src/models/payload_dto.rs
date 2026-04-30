use crate::models::payload_dao::Payload;
use crate::models::status_dto::Status;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;

pub async fn create_payload_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS payloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            status TEXT NOT NULL,
            loc TEXT,
            pid INTEGER NOT NULL DEFAULT 0,
            killed BOOLEAN NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

impl Payload {
    pub async fn add_to_db(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // NOTE: This `loc` will not exist on disk until `prepare` is called!
        let loc_str = self.loc.to_string_lossy();

        let result = sqlx::query("INSERT INTO payloads (status, loc) VALUES (?, ?)")
            .bind(self.status.to_string())
            .bind(loc_str)
            .execute(pool)
            .await?;

        let id = result.last_insert_rowid();
        self.id = id as u32;

        Ok(())
    }

    pub async fn update_status(
        &mut self,
        status: Status,
        pool: &SqlitePool,
    ) -> Result<(), sqlx::Error> {
        let _result = sqlx::query("UPDATE payloads SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(self.id)
            .execute(pool)
            .await?;

        self.status = status;

        Ok(())
    }

    pub async fn update_loc(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let loc_str = self.loc.to_string_lossy();

        sqlx::query("UPDATE payloads SET loc = ? WHERE id = ?")
            .bind(loc_str)
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn update_pid(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE payloads SET pid = ? WHERE id = ?")
            .bind(self.pid)
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn mark_as_killed(&mut self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE payloads SET killed = ? WHERE id = ?")
            .bind(true)
            .bind(self.id)
            .execute(pool)
            .await?;

        self.killed = true;

        Ok(())
    }

    pub async fn retrieve_id(id: u32, pool: &SqlitePool) -> Result<Payload, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM payloads WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;

        let status: String = row.get("status");
        let loc: Option<String> = row.get("loc");

        let mut payload = Payload::new();
        payload.id = row.get("id");
        payload.status = Status::from_string(&status);
        payload.loc = loc.map(PathBuf::from).unwrap_or_default();
        payload.pid = row.get("pid");
        payload.killed = row.get("killed");

        Ok(payload)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_payload_table() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let result = create_payload_table(&pool).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_db() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();

        let result = payload.add_to_db(&pool).await;
        assert!(result.is_ok());
        assert!(payload.id > 0);
    }

    #[tokio::test]
    async fn test_update_status() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();

        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        assert_eq!(payload.status, Status::Unknown);

        payload
            .update_status(Status::Prepared, &pool)
            .await
            .expect("Failed to update payload status");

        assert_eq!(payload.status, Status::Prepared);
    }

    #[tokio::test]
    async fn test_update_pid() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        // Set a PID
        payload.pid = 12345;
        payload.update_pid(&pool).await.unwrap();

        // Retrieve and verify PID was updated
        let retrieved = Payload::retrieve_id(payload_id, &pool).await.unwrap();
        assert_eq!(retrieved.pid, 12345);
    }

    #[tokio::test]
    async fn test_mark_as_killed() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        assert!(!payload.killed);

        payload.mark_as_killed(&pool).await.unwrap();
        assert!(payload.killed);

        // Verify it was persisted to DB
        let retrieved = Payload::retrieve_id(payload_id, &pool).await.unwrap();
        assert!(retrieved.killed);
    }

    #[tokio::test]
    async fn test_update_loc() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        let new_loc = PathBuf::from("/new/location");
        payload.set_loc(new_loc.clone());
        payload.update_loc(&pool).await.unwrap();

        // Retrieve and verify loc was updated
        let retrieved = Payload::retrieve_id(payload_id, &pool).await.unwrap();
        assert_eq!(retrieved.loc, new_loc);
    }

    #[tokio::test]
    async fn test_retrieve_id() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = crate::datasource::db::init_payload_db(db_path.to_str().unwrap()).await;

        let mut payload = Payload::new();

        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");

        let id = payload.id;

        let retrieved_payload = Payload::retrieve_id(id, &pool)
            .await
            .expect("Failed to retrieve payload by ID");

        assert_eq!(retrieved_payload.id, id);
        assert_eq!(retrieved_payload.status, Status::Unknown);
    }
}
