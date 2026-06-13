use crate::models::payload_dao::Payload;
use crate::models::status_dto::Status;
use crate::{routes::router::AppState, utils::io::sanitize_filename};
use axum::response::{IntoResponse, Response};
use axum::{
    extract::{Json, Multipart, Path, State},
    http::{StatusCode, header},
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
        (status = 500, description = "Internal server error"),
    ),
    tag = "files"
)]
pub async fn submit(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    let mut payload = Payload::new();

    // Parse the multipart form data
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                tracing::error!("Multipart error: {e}");
                return (StatusCode::BAD_REQUEST, Json(payload)).into_response();
            }
        };
        if let Some(filename) = field.file_name() {
            let clean_filename = sanitize_filename(filename);
            let data = match field.bytes().await {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Error reading field bytes: {e}");
                    return (StatusCode::BAD_REQUEST, Json(payload)).into_response();
                }
            };
            payload.add_input(clean_filename, data.to_vec());
        }
    }
    // Add job to database
    // TODO: These error responses return empty payloads with no diagnostic info.
    //  They are indicators of an unhealthy client — handle in a future PR.
    let Ok(_) = payload.add_to_db(&state.pool).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(payload)).into_response();
    };

    // FIXME: This sequence can cause a race condition
    // - prepare -> update_loc, if prepare part fails, then `loc` will not be in the DB
    let Ok(_) = payload.prepare(&state.config.data_path) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(payload)).into_response();
    };
    //
    // Update loc in database after prepare() sets it
    let Ok(_) = payload.update_loc(&state.pool).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(payload)).into_response();
    };

    let Ok(_) = payload.update_status(Status::Prepared, &state.pool).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(payload)).into_response();
    };

    (StatusCode::OK, Json(payload)).into_response()
}

#[utoipa::path(
    get,
    path = "/retrieve/{id}",
    params(
        ("id" = i32, Path, description = "Payload identifier")
    ),
    responses(
       (status = 200, description = "Job completed — returns zip file", content_type = "application/zip", body = Vec<u8>),
       (status = 200, description = "Job not yet complete — returns current payload state", body = Payload),
       (status = 404, description = "Payload not found", body = Payload),
       (status = 500, description = "Internal server error", body = Payload),
   ),
    tag = "files"
)]
pub async fn retrieve(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let payload = match Payload::retrieve_id(id, &state.pool).await {
        Ok(p) => p,
        // TODO: Empty payload responses are indicators of an unhealthy client — handle in a future PR.
        Err(e) => {
            let status = match e {
                sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return (status, Json(Payload::new())).into_response();
        }
    };

    match payload.status {
        Status::Completed => match payload.zip_directory() {
            Ok(v) => ([(header::CONTENT_TYPE, "application/zip")], v).into_response(),
            // TODO: Empty payload response is an indicator of an unhealthy client — handle in a future PR.
            Err(e) => {
                tracing::error!("Error compressing directory {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(Payload::new())).into_response()
            }
        },
        _ => Json(payload).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/retrieve_partial/{id}",
    params(
        ("id" = i32, Path, description = "Payload identifier")
    ),
    responses(
       (status = 200, description = "Returns zip file of current payload state, regardless of completion", content_type = "application/zip", body = Vec<u8>),
       (status = 404, description = "Payload not found", body = Payload),
       (status = 500, description = "Internal server error", body = Payload),
   ),
    tag = "files"
)]
pub async fn retrieve_partial(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let payload = match Payload::retrieve_id(id, &state.pool).await {
        Ok(p) => p,
        // TODO: Empty payload responses are indicators of an unhealthy client — handle in a future PR.
        Err(e) => {
            let status = match e {
                sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return (status, Json(Payload::new())).into_response();
        }
    };

    // Always return the zip, regardless of status
    match payload.zip_partial() {
        Ok(v) => ([(header::CONTENT_TYPE, "application/zip")], v).into_response(),
        // TODO: Empty payload response is an indicator of an unhealthy client — handle in a future PR.
        Err(e) => {
            tracing::error!("Error compressing directory {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(Payload::new())).into_response()
        }
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

#[utoipa::path(
    post,
    path = "/kill/{id}",
    params(
    ("id" = u32, Path, description = "ID of payload to be terminated")
    )
)]
pub async fn kill(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let mut payload = match Payload::retrieve_id(id, &state.pool).await {
        Ok(p) => p,
        Err(e) => {
            let status = match e {
                sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return (status, Json(Payload::new())).into_response();
        }
    };

    match payload.kill() {
        Ok(_) => {
            payload.mark_as_killed(&state.pool).await.ok();

            // If the payload hasn't started executing yet (pid == 0, still
            // `Prepared`), there's no process to wait on via the `updater`.
            // Transition it directly to `Killed` so `runner`'s
            // `WHERE status = 'prepared'` filter excludes it and it never
            // gets executed, avoiding an orphaned process later on.
            if payload.pid == 0 {
                payload.update_status(Status::Killed, &state.pool).await.ok();
            }

            (StatusCode::OK).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::loader::{Config, Service};
    use crate::models::payload_dao::Payload;
    use crate::models::payload_dto::create_payload_table;
    use crate::models::status_dto::Status;
    use crate::routes::router::create_client_routes;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sqlx::SqlitePool;
    use std::collections::HashMap;
    use std::fs;
    use std::io::Read;
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        create_payload_table(&pool).await.unwrap();
        pool
    }

    fn make_config(data_path: &str) -> Config {
        let mut services = HashMap::new();
        services.insert(
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
        Config {
            services,
            db_path: ":memory:".to_string(),
            data_path: data_path.to_string(),
            max_age: std::time::Duration::from_secs(3600),
            port: 5000,
        }
    }

    fn build_multipart(boundary: &str, parts: &[(&str, &[u8], Option<&str>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, data, filename) in parts {
            body.extend(format!("--{boundary}\r\n").as_bytes());
            if let Some(fname) = filename {
                body.extend(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{fname}\"\r\n"
                    )
                    .as_bytes(),
                );
                body.extend(b"Content-Type: application/octet-stream\r\n");
            } else {
                body.extend(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
                );
            }
            body.extend(b"\r\n");
            body.extend(*data);
            body.extend(b"\r\n");
        }
        body.extend(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    async fn body_bytes(response: axum::response::Response) -> bytes::Bytes {
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_submit_success() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[("file", b"file content".as_slice(), Some("input.txt"))],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/submit")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body_bytes(response).await;
        let payload: Payload = serde_json::from_slice(&bytes).unwrap();
        assert!(payload.id > 0);
        assert_eq!(payload.status, Status::Prepared);
    }

    #[tokio::test]
    async fn test_retrieve_not_found() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri("/retrieve/9999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_retrieve_non_completed() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload
            .update_status(Status::Prepared, &pool)
            .await
            .unwrap();
        let payload_id = payload.id;

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body_bytes(response).await;
        let retrieved: Payload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(retrieved.status, Status::Prepared);
        assert_eq!(retrieved.id, payload_id);
    }

    #[tokio::test]
    async fn test_retrieve_completed() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        let payload_dir = tempdir.path().join(payload_id.to_string());
        fs::create_dir_all(&payload_dir).unwrap();
        fs::write(payload_dir.join("output.txt"), b"result data").unwrap();

        payload.set_loc(payload_dir);
        payload.update_loc(&pool).await.unwrap();
        payload
            .update_status(Status::Completed, &pool)
            .await
            .unwrap();

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(content_type, "application/zip");
    }

    #[tokio::test]
    async fn test_retrieve_completed_missing_dir() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        // Point loc at a directory that does not exist — zip_directory will fail
        payload.set_loc(tempdir.path().join("does_not_exist"));
        payload.update_loc(&pool).await.unwrap();
        payload
            .update_status(Status::Completed, &pool)
            .await
            .unwrap();

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_retrieve_partial_not_found() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri("/retrieve_partial/9999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_retrieve_partial_non_completed() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        payload
            .update_status(Status::Prepared, &pool)
            .await
            .unwrap();

        // Create payload directory with files
        let payload_dir = tempdir.path().join(payload.id.to_string());
        fs::create_dir_all(&payload_dir).unwrap();
        fs::write(&payload_dir.join("test.txt"), b"partial data").unwrap();
        payload.set_loc(payload_dir);
        payload.update_loc(&pool).await.unwrap();

        let payload_id = payload.id;

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve_partial/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(content_type, "application/zip");

        // Verify the zip contains our test file
        let bytes = body_bytes(response).await;
        assert!(!bytes.is_empty());

        // Unzip and verify contents
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(archive.len() > 0);

        let mut file = archive.by_name("test.txt").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "partial data");
    }

    #[tokio::test]
    async fn test_retrieve_partial_completed() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        let payload_dir = tempdir.path().join(payload_id.to_string());
        fs::create_dir_all(&payload_dir).unwrap();
        fs::write(payload_dir.join("output.txt"), b"result data").unwrap();

        payload.set_loc(payload_dir);
        payload.update_loc(&pool).await.unwrap();
        payload
            .update_status(Status::Completed, &pool)
            .await
            .unwrap();

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve_partial/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(content_type, "application/zip");
    }

    #[tokio::test]
    async fn test_retrieve_partial_zip_error() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        // Set an invalid/non-existent directory
        let invalid_dir = tempdir.path().join("nonexistent");
        payload.set_loc(invalid_dir);
        payload.update_loc(&pool).await.unwrap();

        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/retrieve_partial/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = body_bytes(response).await;
        let payload_resp: Payload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload_resp.id, 0);
    }

    #[tokio::test]
    async fn test_load() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri("/load")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body_bytes(response).await;
        let load: f32 = serde_json::from_slice(&bytes).unwrap();
        assert!(
            load.is_finite(),
            "CPU load should be a finite number, got {load}"
        );
    }

    #[tokio::test]
    async fn test_kill_not_found() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool, config);

        let request = Request::builder()
            .method("POST")
            .uri("/kill/9999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_kill_success() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool.clone(), config);

        // Create a payload
        let mut payload = Payload::new();
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        let request = Request::builder()
            .method("POST")
            .uri(format!("/kill/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify the payload was marked as killed
        let retrieved = Payload::retrieve_id(payload_id, &pool).await.unwrap();
        assert!(retrieved.killed);
    }

    /// Regression test for #56: killing a payload that is still `Prepared`
    /// (pid == 0, i.e. it never started executing) must transition it
    /// directly to `Killed`, not leave it as `Prepared`. Otherwise the
    /// `runner` would later pick it up (since it filters on
    /// `Status::Prepared`), execute it, and the `updater` would mark it as
    /// `Killed` without ever terminating the real spawned process, orphaning
    /// it.
    #[tokio::test]
    async fn test_kill_prepared_payload_transitions_to_killed() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_client_routes(pool.clone(), config.clone());

        // Create a payload that is `Prepared` and has not started (pid == 0)
        let mut payload = Payload::new();
        payload.set_status(Status::Prepared);
        payload.add_to_db(&pool).await.unwrap();
        let payload_id = payload.id;

        let request = Request::builder()
            .method("POST")
            .uri(format!("/kill/{payload_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // The payload must be `Killed`, not left as `Prepared`
        let retrieved = Payload::retrieve_id(payload_id, &pool).await.unwrap();
        assert_eq!(retrieved.status, Status::Killed);
        assert!(retrieved.killed);
        assert_eq!(retrieved.pid, 0);

        // Regression check: `runner` filters on `Status::Prepared`, so it
        // must no longer pick up this payload (which would otherwise spawn
        // the real process and orphan it once the updater short-circuits on
        // `is_killed()`).
        let mut queue = crate::models::queue_dao::PayloadQueue::new(&config);
        queue
            .list_per_status(Status::Prepared, &pool)
            .await
            .unwrap();
        assert!(
            !queue.jobs.iter().any(|p| p.id == payload_id),
            "runner should not pick up a payload that was killed before execution"
        );
    }
}
