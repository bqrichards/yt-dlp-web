use tempfile::env;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, instrument};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

use axum::{
    Json, Router,
    body::Body,
    extract::Query,
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

mod error;
mod title;
mod video;

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry().with(fmt::layer()).init();

    let api = Router::new()
        .route("/queue", get(queue_video))
        .route("/download", get(download_video));

    let static_dir = ServeDir::new("static");
    let app = Router::new()
        .route("/health", get(healthcheck))
        .nest("/api", api)
        .fallback_service(static_dir);

    let addr = format!("0.0.0.0:{}", get_port());
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[instrument]
async fn healthcheck() -> &'static str {
    "OK"
}

#[derive(Deserialize, Debug)]
struct QueueVideoRequest {
    url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct VideoObject {
    title: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DownloadVideoResponse {
    videos: Vec<VideoObject>,
}

#[instrument]
async fn queue_video(
    Query(payload): Query<QueueVideoRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let url = payload.url.as_str();
    let (video_titles, _videos_downloaded) =
        tokio::join!(title::get_video_titles(url), video::download_videos(url));
    let titles = video_titles.map_err(|e| {
        error!("titles error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error downloading video stream",
        )
            .into_response()
    })?;

    let videos: Vec<VideoObject> = titles
        .iter()
        .map(|f| VideoObject {
            title: f.to_string(),
        })
        .collect();

    let body = DownloadVideoResponse { videos };
    Ok(Json(body).into_response())
}

#[instrument]
async fn download_video(
    Query(payload): Query<VideoObject>,
) -> Result<Response<Body>, Response<Body>> {
    let filename = payload.title;
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
        format!("attachment; filename={}", &filename)
            .parse()
            .unwrap(),
    );

    debug!("{:?}", headers);

    Ok((headers, body).into_response())
}
