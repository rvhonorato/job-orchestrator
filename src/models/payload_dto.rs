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
        let loc_str = self.loc.to_str().ok_or_else(|| {
            sqlx::Error::Protocol(
                "Invalid loc path: contains invalid UTF-8 and cannot be converted to string"
                    .to_string(),
            )
        })?;

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
        let loc_str = self.loc.to_str().ok_or_else(|| {
            sqlx::Error::Protocol(
                "Invalid loc path: contains invalid UTF-8 and cannot be converted to string"
                    .to_string(),
            )
        })?;

        sqlx::query("UPDATE payloads SET loc = ? WHERE id = ?")
            .bind(loc_str)
            .bind(self.id)
            .execute(pool)
            .await?;

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

        Ok(payload)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn test_add_to_db() {
        let pool = crate::datasource::db::init_payload_db().await;

        let mut payload = Payload::new();

        let result = payload.add_to_db(&pool).await;
        assert!(result.is_ok());
        assert!(payload.id > 0);
    }

    #[tokio::test]
    async fn test_update_status() {
        let pool = crate::datasource::db::init_payload_db().await;

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
    async fn test_retrieve_id() {
        let pool = crate::datasource::db::init_payload_db().await;

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
