use crate::config::loader::Config;
use crate::controllers::client::{load, retrieve, submit};
use crate::controllers::health::__path_health;
use crate::controllers::health::health;
use crate::controllers::orchestrator::__path_download;
use crate::controllers::orchestrator::__path_upload;
use crate::controllers::orchestrator::{download, upload};
use crate::controllers::ping::ping;
use crate::models::health_dto::Health;
use crate::models::job_dao::Job;
use axum::extract::DefaultBodyLimit;
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::trace::{
    DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer,
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        upload,
        download,
        health
    ),
    components(
        schemas(Job, Health)
    ),
    tags(
        (name = "files", description = "File management endpoints"),
        (name = "health", description = "Health check endpoints")
    )
)]
struct ApiDoc;

pub fn create_routes(pool: SqlitePool, config: Config) -> Router {
    let state = AppState { pool, config };
    Router::new()
        .route("/", get(ping))
        .route("/health", get(health))
        .route("/upload", post(upload))
        .route("/download/{id}", get(download))
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(true), // Log request headers
                )
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true), // Log response headers
                )
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
        .layer(DefaultBodyLimit::max(400 * 1024 * 1024)) // Set max body size to 400MB
}

pub fn create_client_routes(pool: SqlitePool, config: Config) -> Router {
    let state = AppState { pool, config };
    Router::new()
        .route("/", get(ping))
        .route("/load", get(load))
        .route("/submit", post(submit))
        .route("/retrieve/{id}", get(retrieve))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(true), // Log request headers
                )
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true), // Log response headers
                )
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
        .layer(DefaultBodyLimit::disable())
}
