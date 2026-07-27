use axum::{
    Router,
    routing::{any, get},
};
use tower_http::services::ServeDir;

use crate::{
    endpoint::{download_video_endpoint, healthcheck_endpoint},
    ws::ws_handler,
};

pub fn app_router() -> Router {
    let static_dir = ServeDir::new("static");
    Router::new()
        .route("/health", get(healthcheck_endpoint))
        .nest("/api", api_router())
        .fallback_service(static_dir)
}

pub fn api_router() -> Router {
    Router::new()
        .route("/ws", any(ws_handler))
        .route("/download", get(download_video_endpoint))
}
