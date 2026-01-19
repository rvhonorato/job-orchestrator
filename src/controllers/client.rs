use crate::{routes::router::AppState, utils::io::sanitize_filename};

use crate::models::payload_dao::Payload;
use crate::models::status_dto::Status;
use axum::{
    extract::{Json, Multipart, Path, State},
    http::StatusCode,
};
use sysinfo::System;

#[utoipa::path(
    post,
    path = "/submit",
    request_body(
        content_type = "multipart/form-data",
    ),
    responses(
        (status = 200, description = "File uploaded successfully", body = Payload),
        // (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
        // (status = 503, description = "Service unavailable")
    ),
    tag = "files"
)]
pub async fn submit(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Payload>, (StatusCode, String)> {
    let mut payload = Payload::new();

    // Parse the multipart form data
    while let Some(field) = multipart.next_field().await.unwrap() {
        if let Some(filename) = field.file_name() {
            let clean_filename = sanitize_filename(filename);
            let data = field.bytes().await.unwrap();

            payload.add_input(clean_filename, data.to_vec());
        }
    }

    payload
        .add_to_db(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // FIXME: This sequence can cause a race condition
    // - prepare -> update_loc, if prepare part fails, then `loc` will not be in the DB
    payload.prepare(&state.config.data_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to prepare payload: {e}"),
        )
    })?;

    // Update loc in database after prepare() sets it
    payload
        .update_loc(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    payload
        .update_status(Status::Prepared, &state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(payload))
}

#[utoipa::path(
    get,
    path = "/retrieve/{id}",
    params(
        ("id" = i32, Path, description = "Payload identifier")
    ),
    responses(
        (status = 200, description = "File downloaded successfully", body = Vec<u8>),
        (status = 202, description = "Job not ready"),
        (status = 204, description = "Job failed or cleaned"),
        (status = 404, description = "Job not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "files"
)]
pub async fn retrieve(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Vec<u8>, StatusCode> {
    let payload = Payload::retrieve_id(id, &state.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    match payload.status {
        Status::Completed => match payload.zip_directory() {
            Ok(v) => Ok(v),
            Err(e) => {
                tracing::error!("Error compressing directory {:?}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Status::Invalid => Err(StatusCode::BAD_REQUEST),
        Status::Failed => Err(StatusCode::INTERNAL_SERVER_ERROR),
        Status::Cleaned => Err(StatusCode::NO_CONTENT),
        _ => Err(StatusCode::ACCEPTED),
    }
}

#[utoipa::path(
    get,
    path = "/load",
    responses(
        (status = 200, description = "Get the load of the client", body = f32),
    ),
)]
pub async fn load() -> Json<f32> {
    // TODO: Implement cached background monitoring of CPU load
    let mut sys = System::new();

    // Measure delta
    sys.refresh_cpu_all();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_all();

    Json(sys.global_cpu_usage())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Config;
    use crate::routes::router::AppState;
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::{routing::get, routing::post, Router};
    use http::{header, Request, StatusCode};
    use sqlx::SqlitePool;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tower::ServiceExt; // for `oneshot`
    use uuid::Uuid;

    // Helper function to initialize the database schema
    pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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

    // Helper functions to create multipart form data
    fn form_text_file(boundary: &str, name: &str, filename: &str, content: &str) -> Vec<u8> {
        let mut part = format!(
            "--{boundary}\r\n\
                Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
                Content-Type: text/plain\r\n\r\n"
        )
        .into_bytes();
        part.extend_from_slice(content.as_bytes());
        part.extend_from_slice(b"\r\n");
        part
    }
    fn form_file(
        boundary: &str,
        name: &str,
        filename: &str,
        content_type: &str,
        content: &[u8],
    ) -> Vec<u8> {
        let mut part = format!(
            "--{boundary}\r\n\
                Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
                Content-Type: {content_type}\r\n\r\n"
        )
        .into_bytes();
        part.extend_from_slice(content);
        part.extend_from_slice(b"\r\n");
        part
    }

    async fn setup_submit_test_router(endpoint: &str) -> (Router, Config) {
        // Setup the route
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap(); // Initialize the database schema
        let state = AppState {
            pool,
            config: config.clone(),
        };

        (
            Router::new()
                .route(endpoint, post(submit))
                .with_state(state),
            config,
        )
    }

    async fn setup_retrieve_test_router(endpoint: &str) -> (Router, u32, u32, tempfile::TempDir) {
        // Setup the route - keep tempdir alive by returning it
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap(); // Initialize the database schema
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        // Make a completed payload in the database
        // This simulates the full workflow: add input, get DB id, prepare files, mark as completed
        let mut payload = Payload::new();
        payload.add_input(
            "test01.txt".to_string(),
            b"hello this is a test file".to_vec(),
        );

        // Set a temporary ID to create the directory structure
        payload.id = 1;
        let payload_loc = PathBuf::from(&config.data_path).join("1");
        payload.set_loc(payload_loc.clone());

        // Manually create directory and files (simulating prepare)
        std::fs::create_dir_all(&payload_loc).expect("Failed to create payload directory");
        std::fs::write(payload_loc.join("test01.txt"), b"hello this is a test file")
            .expect("Failed to write test file");

        // Now add to DB with the loc already set
        payload
            .add_to_db(&state.pool)
            .await
            .expect("Failed to add payload to DB");

        payload
            .update_status(Status::Completed, &pool)
            .await
            .expect("Failed to update status");
        let complete_jobid = payload.id;

        // Make a failed payload in the database
        // For a failed job, we still need a valid loc in DB
        let mut payload = Payload::new();
        payload.set_id(2);
        payload
            .prepare(&state.config.data_path)
            .expect("Failed to prepare payload");

        payload
            .add_to_db(&state.pool)
            .await
            .expect("Failed to add payload to DB");

        payload
            .update_status(Status::Failed, &pool)
            .await
            .expect("Failed to update status");
        let failed_jobid = payload.id;

        (
            Router::new()
                .route(endpoint, get(retrieve))
                .with_state(state),
            complete_jobid,
            failed_jobid,
            data_dir, // Return tempdir to keep it alive
        )
    }

    #[tokio::test]
    async fn test_submit() {
        let endpoint = "/submit";
        let (test_app, _) = setup_submit_test_router(endpoint).await;

        // Create a multipart/form-data request
        let boundary = format!("----Boundary{}", Uuid::new_v4());
        let mut body = Vec::new();
        body.extend(form_text_file(
            &boundary,
            "file",
            "test01.txt",
            "hello this is a test file",
        ));
        body.extend(form_file(
            &boundary,
            "file",
            "test.dat",
            "application/octet-stream",
            b"\x00\x01\x02\x03",
        ));
        body.extend(format!("--{boundary}--\r\n").as_bytes());

        // Create the request
        let req = Request::builder()
            .method("POST")
            .uri(endpoint)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        // Make the request
        let response = test_app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["id"], 1);
        assert_eq!(json["status"], String::from("Prepared"));

        // Check if the file was saved correctly
        let expected_loc = json["loc"].as_str().unwrap();
        let expected_file = PathBuf::from(expected_loc).join("test01.txt");
        assert!(expected_file.exists());
        let expected_file = PathBuf::from(expected_loc).join("test.dat");
        assert!(expected_file.exists());
    }

    #[tokio::test]
    async fn test_retrieve() {
        let (test_app, valid_jobid, _, _tempdir) =
            setup_retrieve_test_router("/retrieve/{id}").await;
        let endpoint = format!("/retrieve/{}", valid_jobid);

        let req = Request::builder()
            .method("GET")
            .uri(endpoint)
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            test_app.oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );
    }
    #[tokio::test]
    async fn test_retrieve_failed_returns_internal_server_error() {
        let (test_app, _, failed_jobid, _tempdir) =
            setup_retrieve_test_router("/retrieve/{id}").await;
        let endpoint = format!("/retrieve/{}", failed_jobid);

        let req = Request::builder()
            .method("GET")
            .uri(endpoint)
            .body(Body::empty())
            .unwrap();

        // Failed status (system error) returns 500 Internal Server Error
        assert_eq!(
            test_app.oneshot(req).await.unwrap().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_retrieve_invalid_returns_bad_request() {
        // Setup
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        // Create an invalid payload (user error - e.g., missing run.sh)
        let mut payload = Payload::new();
        payload.set_id(1);
        payload
            .prepare(&config.data_path)
            .expect("Failed to prepare payload");
        payload
            .add_to_db(&pool)
            .await
            .expect("Failed to add payload to DB");
        payload
            .update_status(Status::Invalid, &pool)
            .await
            .expect("Failed to update status");

        let test_app = Router::new()
            .route("/retrieve/{id}", get(retrieve))
            .with_state(state);

        let endpoint = format!("/retrieve/{}", payload.id);
        let req = Request::builder()
            .method("GET")
            .uri(endpoint)
            .body(Body::empty())
            .unwrap();

        // Invalid status (user error) returns 400 Bad Request
        assert_eq!(
            test_app.oneshot(req).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_retrieve_not_found() {
        let (test_app, _, _, _tempdir) = setup_retrieve_test_router("/retrieve/{id}").await;
        let endpoint = "/retrieve/999";

        // Create the request
        let req = Request::builder()
            .method("GET")
            .uri(endpoint)
            .body(Body::empty())
            .unwrap();

        // Make the request
        let response = test_app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_load() {
        // Setup the route - no state needed since load() doesn't use AppState
        let test_app = Router::new().route("/load", get(load));

        // Create the request
        let req = Request::builder()
            .method("GET")
            .uri("/load")
            .body(Body::empty())
            .unwrap();

        // Make the request
        let response = test_app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Parse the response body
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let cpu_usage: f32 = serde_json::from_slice(&body).unwrap();

        // CPU usage should be a valid percentage (0.0 to 100.0+)
        // Note: Can occasionally be slightly over 100 on some systems
        assert!(cpu_usage >= 0.0, "CPU usage should be non-negative");
        assert!(cpu_usage <= 200.0, "CPU usage should be reasonable");
    }

    // ===== Additional submit tests =====

    #[tokio::test]
    async fn test_submit_no_files() {
        let endpoint = "/submit";
        let (test_app, _) = setup_submit_test_router(endpoint).await;

        let boundary = format!("----Boundary{}", Uuid::new_v4());
        let mut body = Vec::new();
        // No files, just end boundary
        body.extend(format!("--{boundary}--\r\n").as_bytes());

        let req = Request::builder()
            .method("POST")
            .uri(endpoint)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = test_app.oneshot(req).await.unwrap();
        // Should succeed even with no files
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_submit_sanitizes_filename() {
        let endpoint = "/submit";
        let (test_app, _) = setup_submit_test_router(endpoint).await;

        let boundary = format!("----Boundary{}", Uuid::new_v4());
        let mut body = Vec::new();
        // File with path traversal attempt
        body.extend(form_text_file(
            &boundary,
            "file",
            "../../../etc/passwd",
            "malicious content",
        ));
        body.extend(format!("--{boundary}--\r\n").as_bytes());

        let req = Request::builder()
            .method("POST")
            .uri(endpoint)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = test_app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let payload_loc = json["loc"].as_str().unwrap();

        // Verify the filename was sanitized to just "passwd"
        let saved_file = PathBuf::from(payload_loc).join("passwd");
        assert!(saved_file.exists());
    }

    // ===== Additional retrieve tests =====

    #[tokio::test]
    async fn test_retrieve_pending_status() {
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        let mut payload = Payload::new();
        payload
            .prepare(&state.config.data_path)
            .expect("Failed to prepare");
        payload.add_to_db(&pool).await.expect("Failed to add to db");
        payload
            .update_status(Status::Pending, &pool)
            .await
            .expect("Failed to update status");

        let app = Router::new()
            .route("/retrieve/{id}", get(retrieve))
            .with_state(state);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{}", payload.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_retrieve_processing_status() {
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        let mut payload = Payload::new();
        payload
            .prepare(&state.config.data_path)
            .expect("Failed to prepare");
        payload.add_to_db(&pool).await.expect("Failed to add to db");
        payload
            .update_status(Status::Processing, &pool)
            .await
            .expect("Failed to update status");

        let app = Router::new()
            .route("/retrieve/{id}", get(retrieve))
            .with_state(state);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{}", payload.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_retrieve_cleaned_status() {
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        let mut payload = Payload::new();
        payload
            .prepare(&state.config.data_path)
            .expect("Failed to prepare");
        payload.add_to_db(&pool).await.expect("Failed to add to db");
        payload
            .update_status(Status::Cleaned, &pool)
            .await
            .expect("Failed to update status");

        let app = Router::new()
            .route("/retrieve/{id}", get(retrieve))
            .with_state(state);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{}", payload.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_retrieve_prepared_status() {
        let data_dir = tempdir().unwrap();
        let mut config = Config::new().unwrap();
        config.data_path = data_dir.path().to_str().unwrap().to_string();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_db(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            config: config.clone(),
        };

        let mut payload = Payload::new();
        payload
            .prepare(&state.config.data_path)
            .expect("Failed to prepare");
        payload.add_to_db(&pool).await.expect("Failed to add to db");
        payload
            .update_status(Status::Prepared, &pool)
            .await
            .expect("Failed to update status");

        let app = Router::new()
            .route("/retrieve/{id}", get(retrieve))
            .with_state(state);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{}", payload.id))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }
}
