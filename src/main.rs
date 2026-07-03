use std::{net::SocketAddr, sync::OnceLock};

use regex::Regex;
use tempfile::env;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{debug, info, instrument};
use tracing_subscriber::{filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::{any, get},
};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

use crate::ws::{MediaFormat, ws_handler};

mod error;
mod video;
mod ws;

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(filter::LevelFilter::DEBUG)
        .with(fmt::layer())
        .init();

    let api = Router::new()
        .route("/ws", any(ws_handler))
        .route("/download", get(download_video));

    let static_dir = ServeDir::new("static");
    let app = Router::new()
        .route("/health", get(healthcheck))
        .nest("/api", api)
        .fallback_service(static_dir);

    let addr = format!("0.0.0.0:{}", get_port());
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

#[instrument]
async fn healthcheck() -> &'static str {
    "OK"
}

#[derive(Serialize, Deserialize, Debug)]
struct VideoObject {
    id: String,
    media_format: MediaFormat,
}

static VIDEO_ID_RE: OnceLock<Regex> = OnceLock::new();

fn video_id_re() -> &'static Regex {
    VIDEO_ID_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{1,64}$").unwrap())
}

#[instrument]
async fn download_video(
    Query(payload): Query<VideoObject>,
) -> Result<Response<Body>, Response<Body>> {
    let VideoObject { id, media_format } = payload;
    if !video_id_re().is_match(&id) {
        return Err((StatusCode::BAD_REQUEST, "invalid video id").into_response());
    }
    let ext = match media_format {
        MediaFormat::Audio => "mp3",
        MediaFormat::Video => "mp4",
    };

    let filename = format!("{}.{}", id, ext);
    let mut path = env::temp_dir();
    path.push(&filename);
    debug!("Reading file: {:?}", path);

    let file = File::open(path.clone()).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            format!("failed to open file {:?}: {}", path, e),
        )
            .into_response()
    })?;

    let metadata = file.metadata().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("metadata error: {}", e),
        )
            .into_response()
    })?;
    let content_length = metadata.len();

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str("application/octet-stream").unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str("attachment").unwrap(),
    );

    debug!("{:?}", headers);

    Ok((headers, body).into_response())
}
