use crate::models::health_dto::Health;
use crate::routes::router::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = Health),
        (status = 503, description = "Service is unhealthy")
    ),
    tag = "health"
)]
pub async fn health(State(state): State<AppState>) -> Result<Json<Health>, StatusCode> {
    // Check database connectivity
    let db_status = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => "ok",
        Err(_) => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    Ok(Json(Health {
        status: "ok".to_string(),
        database: db_status.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Config;
    use crate::routes::router::AppState;
    use axum::extract::State;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn test_health_returns_ok() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let config = Config::new().unwrap();
        let state = State(AppState { pool, config });

        let response = health(state).await;
        assert!(response.is_ok());

        let health_response = response.unwrap().0;
        assert_eq!(health_response.status, "ok");
        assert_eq!(health_response.database, "ok");
    }

    #[tokio::test]
    async fn test_health_database_failure() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let config = Config::new().unwrap();

        // Close the pool to simulate database unavailability
        pool.close().await;

        let state = State(AppState { pool, config });

        let response = health(state).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_health_invalid_connection() {
        // Try to connect to an invalid database path
        // This test verifies behavior when the database is genuinely unavailable
        let result = SqlitePool::connect("sqlite:///nonexistent/path/db.sqlite").await;

        // The connection itself should fail for an invalid path
        assert!(result.is_err());
    }
}
