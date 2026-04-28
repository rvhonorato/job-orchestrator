use crate::models::job_dao::Job;
use crate::models::status_body::StatusBody;
use crate::models::status_dto::Status;
use crate::routes::router::AppState;
use crate::services::server;
use crate::utils::io::{sanitize_filename, save_file};
use axum::response::{IntoResponse, Response};
use axum::{
    extract::{Json, Multipart, Path, State},
    http::{StatusCode, header},
};
use std::collections::HashMap;
use tokio::fs::create_dir_all;
use utoipa;

#[utoipa::path(
    get,
    path = "/download/{id}",
    params(
        ("id" = u32, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "Job completed — returns zip file", content_type = "application/zip", body = Vec<u8>),
        (status = 200, description = "Job not yet complete — returns current job status", body = StatusBody),
        (status = 404, description = "Not found", body = StatusBody),
        (status = 500, description = "Internal server error", body = StatusBody),
    ),
    tag = "files"
)]
pub async fn download(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let mut job = Job::new(&state.config.data_path);
    let mut body = StatusBody::new();

    let result = job.retrieve_id(id, &state.pool).await;

    if let Err(e) = result {
        // Error when retrieving this id, check what kind of error it was
        let status = match e {
            // It is not found on the database
            sqlx::Error::RowNotFound => {
                body.message = format!("Job {id} not found in the database");
                StatusCode::NOT_FOUND
            }
            // Something else
            _ => {
                body.message = "Internal server error".to_string();
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        // Return the error
        return (status, Json(body)).into_response();
    }

    body.id = job.id;
    body.status = job.status;

    match job.status {
        Status::Completed => match job.download() {
            Ok(data) => ([(header::CONTENT_TYPE, "application/zip")], data).into_response(),
            Err(e) => {
                tracing::error!("Error reading output file: {:?}", e);
                body.message = "Error reading output file".to_string();
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
            }
        },
        _ => Json(body).into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/upload",
    request_body(
        content_type = "multipart/form-data",
        description = "Upload a file and metadata fields as multipart/form-data. \
        The request must include a file field (with any filename and content type), a 'user_id' field (integer), and a 'service' field (string). \
        Additional fields may be included as needed."
    ),
    responses(
        (status = 201, description = "File uploaded successfully", body = StatusBody),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "files"
)]
pub async fn upload(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    // Create a new job with unique ID
    let mut job = Job::new(&state.config.data_path);

    // Create job directory
    if create_dir_all(&job.loc).await.is_err() {
        let mut body = StatusBody::new();
        body.message = "Could not create job directory".to_string();
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
    }

    // FIXME: Move the parsing of the form to a helper function
    let mut text_fields = HashMap::new();
    let mut file_count = 0;
    let mut body = StatusBody::new();
    loop {
        let field = multipart.next_field().await;
        let field = match field {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                tracing::error!("Multipart error: {e}");
                body.message = format!("Multipart error: {e}");
                return (StatusCode::BAD_REQUEST, Json(body)).into_response();
            }
        };

        let field_name = field.name().unwrap_or("unnamed").to_string();

        if let Some(filename) = field.file_name() {
            file_count += 1;
            let filename = sanitize_filename(filename);
            let file_path = job.loc.join(&filename);

            tracing::info!("Saving file: {} to {}", filename, file_path.display());

            if let Err((err_code, err_msg)) = save_file(field, &file_path).await {
                body.message = format!("Could not save file: {err_msg}");
                return (err_code, Json(body)).into_response();
            }
        } else {
            let text = match field.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Error reading text field: {e}");
                    body.message = format!("Error reading text field: {e}");
                    return (StatusCode::BAD_REQUEST, Json(body)).into_response();
                }
            };
            text_fields.insert(field_name, text);
        }
    }

    tracing::info!(
        "Upload completed: {} files saved, {} text fields",
        file_count,
        text_fields.len()
    );
    // Now handle special fields

    // Get the user id from the request
    let uid_str = match text_fields.get("user_id") {
        Some(v) => v,
        None => {
            body.message = "Missing user_id field".to_string();
            return (StatusCode::BAD_REQUEST, Json(body)).into_response();
        }
    };

    // Check if this user id is valid, it must be a number
    let uid = match uid_str.parse::<i32>() {
        Ok(v) => v,
        Err(_) => {
            body.message = "Invalid user_id, should be a number".to_string();
            return (StatusCode::BAD_REQUEST, Json(body)).into_response();
        }
    };

    // Get service from the request
    let service = match text_fields.get("service") {
        Some(v) => v,
        None => {
            body.message = "Missing service field".to_string();
            return (StatusCode::BAD_REQUEST, Json(body)).into_response();
        }
    };

    // Validate service exists
    if !state.config.services.contains_key(service) {
        body.message = "Invalid service".to_string();
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    }

    job.set_user_id(uid);
    job.set_service(service.to_string());

    // Add job to database
    let Ok(_) = job.add_to_db(&state.pool).await else {
        body.message = "Error while adding the job to the database".to_string();
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
    };

    let Ok(_) = job.update_status(Status::Queued, &state.pool).await else {
        body.message = format!("Could not update the status of job {0}", job.id);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
    };

    // Everything went fine
    body.status = job.status;
    body.id = job.id;
    body.message = "Job successfully uploaded".to_string();

    (StatusCode::CREATED, Json(body)).into_response()
}

#[utoipa::path(
    get,
    path = "/terminate/{id}",
    params(
        ("id" = u32, Path, description = "Job identifier")
    ),
    responses(),
    tag = "files"
)]
pub async fn terminate(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    // 1. Get a job from the id
    let mut job = Job::new(&state.config.data_path);
    let mut body = StatusBody::new();
    let result = job.retrieve_id(id, &state.pool).await;
    if let Err(e) = result {
        // Error when retrieving this id, check what kind of error it was
        let status = match e {
            // It is not found on the database
            sqlx::Error::RowNotFound => {
                body.message = format!("Job {id} not found in the database");
                StatusCode::NOT_FOUND
            }
            // Something else
            _ => {
                body.message = "Internal server error".to_string();
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        // Return the error
        return (status, Json(body)).into_response();
    }

    body.id = job.id;

    // 2. Send termination signal to client
    let status = match server::terminate_job(job, state.pool.clone(), state.config.clone()).await {
        Ok(_) => {
            body.message = "job terminated".to_string();
            StatusCode::OK
        }
        Err(_) => {
            body.message = "could not terminate job".to_string();
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };

    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use crate::config::loader::{Config, Service};
    use crate::models::job_dao::Job;
    use crate::models::job_dto::create_jobs_table;
    use crate::models::status_body::StatusBody;
    use crate::models::status_dto::Status;
    use crate::routes::router::create_routes;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sqlx::SqlitePool;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        create_jobs_table(&pool).await.unwrap();
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
    async fn test_upload_success() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[
                ("file", b"file content".as_slice(), Some("test.txt")),
                ("user_id", b"1", None),
                ("service", b"test", None),
            ],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.status, Status::Queued);
        assert!(body.message.contains("Job successfully uploaded"));
    }

    #[tokio::test]
    async fn test_upload_missing_user_id() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[
                ("file", b"file content".as_slice(), Some("test.txt")),
                ("service", b"test", None),
            ],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(body.message.contains("Missing user_id"));
    }

    #[tokio::test]
    async fn test_upload_invalid_user_id() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[
                ("file", b"file content".as_slice(), Some("test.txt")),
                ("user_id", b"abc", None),
                ("service", b"test", None),
            ],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(body.message.contains("Invalid user_id"));
    }

    #[tokio::test]
    async fn test_upload_missing_service() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[
                ("file", b"file content".as_slice(), Some("test.txt")),
                ("user_id", b"1", None),
            ],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(body.message.contains("Missing service"));
    }

    #[tokio::test]
    async fn test_upload_invalid_service() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let boundary = "testboundary123";
        let body = build_multipart(
            boundary,
            &[
                ("file", b"file content".as_slice(), Some("test.txt")),
                ("user_id", b"1", None),
                ("service", b"unknownservice", None),
            ],
        );

        let request = Request::builder()
            .method("POST")
            .uri("/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(body.message.contains("Invalid service"));
    }

    #[tokio::test]
    async fn test_download_not_found() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());
        let app = create_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri("/download/9999")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(
            body.message.to_lowercase().contains("not found") && body.message.contains("9999"),
            "Expected 'not found' message mentioning job id, got: {}",
            body.message
        );
    }

    #[tokio::test]
    async fn test_download_queued_job() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Queued, &pool).await.unwrap();
        let job_id = job.id;

        let app = create_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/download/{job_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.id, job_id);
        assert_eq!(body.status, Status::Queued);
    }

    #[tokio::test]
    async fn test_download_running_job() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();
        job.update_status(Status::Running, &pool).await.unwrap();
        let job_id = job.id;

        let app = create_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/download/{job_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.id, job_id);
        assert_eq!(body.status, Status::Running);
    }

    #[tokio::test]
    async fn test_download_completed_job() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();

        fs::create_dir_all(&job.loc).unwrap();
        fs::write(job.loc.join("output.zip"), b"fake zip content").unwrap();

        job.update_status(Status::Completed, &pool).await.unwrap();
        let job_id = job.id;

        let app = create_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/download/{job_id}"))
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

        let bytes = body_bytes(response).await;
        assert_eq!(&bytes[..], b"fake zip content");
    }

    #[tokio::test]
    async fn test_download_completed_missing_file() {
        let tempdir = TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let config = make_config(tempdir.path().to_str().unwrap());

        let mut job = Job::new(tempdir.path().to_str().unwrap());
        job.set_user_id(1);
        job.set_service("test".to_string());
        job.add_to_db(&pool).await.unwrap();
        // No directory or output.zip created on disk
        job.update_status(Status::Completed, &pool).await.unwrap();
        let job_id = job.id;

        let app = create_routes(pool, config);

        let request = Request::builder()
            .method("GET")
            .uri(format!("/download/{job_id}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = body_bytes(response).await;
        let body: StatusBody = serde_json::from_slice(&bytes).unwrap();
        assert!(
            body.message.contains("output file"),
            "Expected error about output file, got: {}",
            body.message
        );
    }
}
