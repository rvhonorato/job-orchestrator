use crate::models::body_dao::StatusBody;
use crate::models::job_dao::Job;
use crate::models::status_dto::Status;
use crate::routes::router::AppState;
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
           ("id" = i32, Path, description = "Job identifier")
       ),
       responses(
           (status = 200, description = "Job completed — returns zip file", content_type = "application/zip",
  body = Vec<u8>),
           (status = 200, description = "Job not yet complete — returns current job status", body = StatusBody),
           (status = 404, description = "Not found", body = StatusBody),
           (status = 500, description = "Internal server error", body = StatusBody),
       ),
       tag = "files"
   )]
pub async fn download(State(state): State<AppState>, Path(id): Path<i32>) -> Response {
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
    body.status = job.status.clone(); // FIXME: This clone

    match job.status {
        Status::Completed => {
            ([(header::CONTENT_TYPE, "application/zip")], job.download()).into_response()
        }
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
        (status = 200, description = "File uploaded successfully", body = StatusBody),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "files"
)]
pub async fn upload(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    // Create a new job with unique ID
    let mut job = Job::new(&state.config.data_path);

    // Create job directory
    let _ = create_dir_all(&job.loc).await.map_err(|_| {
        let mut body = StatusBody::new();
        body.message = "Could not create job directory".to_string();
        (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
    });

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
