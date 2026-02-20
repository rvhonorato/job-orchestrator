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
