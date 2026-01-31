use std::{
    io::{self},
    string::FromUtf8Error,
};

use tempfile::env;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, instrument};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use urlencoding::encode;

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{HeaderMap, Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use tokio::{fs::File, process::Command};
use tower_http::services::ServeDir;
use uuid::Uuid;

fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry().with(fmt::layer()).init();

    let api = Router::new().route("/download", get(download_video));

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
struct DownloadVideoRequest {
    url: String,
}

#[instrument]
async fn download_video(
    Query(payload): Query<DownloadVideoRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let url = payload.url.as_str();
    let filename = match get_video_title(url).await {
        Ok(title) => encode(title.as_str()).into_owned(),
        Err(e) => {
            error!("Failed to get title, defaulting: {:?}", e);
            "video".to_string()
        }
    };
    let stream = get_video_stream(url).await.map_err(|e| {
        error!("Error when downloading video: {:?}", e);

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error downloading video stream",
        )
            .into_response()
    })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename={}", filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );

    debug!("{:?}", headers);

    let body = Body::from_stream(stream);
    Ok((headers, body).into_response())
}

#[derive(thiserror::Error, Debug)]
enum DownloadError {
    #[error("failed to run title command")]
    TitleCommand(#[source] io::Error),
    #[error("failed to run video command")]
    VideoCommand(#[source] io::Error),
    #[error("video download command exited with no status code")]
    VideoExitNoCode,
    #[error("video download command exited with status code {0}")]
    VideoExitErrorCode(i32),
    #[error("title download command exited with no status code")]
    TitleExitNoCode,
    #[error("title download command exited with status code {0}")]
    TitleExitErrorCode(i32),
    #[error("failed to open temp file")]
    TempFileOpen(#[source] io::Error),
    #[error("UTF-8 conversion failed")]
    FromUtf8(#[source] FromUtf8Error),
}

#[instrument]
async fn get_video_title(url: &str) -> Result<String, DownloadError> {
    let cmd = Command::new("yt-dlp")
        .arg("-S")
        .arg("res,ext:mp4:m4a")
        .arg("--recode")
        .arg("mp4")
        .arg("--print")
        .arg("filename")
        .arg(url)
        .output()
        .await
        .map_err(|e| DownloadError::TitleCommand(e))?;

    debug!("Command status: {}", cmd.status);
    let code: Result<i32, DownloadError> = match cmd.status.code() {
        Some(code) => match code {
            0 => Ok(0),
            _ => Err(DownloadError::TitleExitErrorCode(code)),
        },
        None => Err(DownloadError::TitleExitNoCode),
    };
    code?;

    let title = String::from_utf8(cmd.stdout)
        .map(|s| String::from(s.trim()))
        .map_err(|e| DownloadError::FromUtf8(e))?;

    Ok(title)
}

#[instrument]
async fn get_video_stream(url: &str) -> Result<ReaderStream<File>, DownloadError> {
    let mut path = env::temp_dir();
    path.push(format!("ytdlp-web-{}.mp4", Uuid::new_v4()));
    debug!("Temp File Path: {:?}", path);

    let cmd = Command::new("yt-dlp")
        .arg("-S")
        .arg("res,ext:mp4:m4a")
        .arg("--recode")
        .arg("mp4")
        .arg("-o")
        .arg(&path)
        .arg(url)
        .output()
        .await
        .map_err(|e| DownloadError::VideoCommand(e))?;

    debug!("Command status: {}", cmd.status);
    let stdout = String::from_utf8(cmd.stdout).map_err(|e| DownloadError::FromUtf8(e))?;
    let stderr = String::from_utf8(cmd.stderr).map_err(|e| DownloadError::FromUtf8(e))?;
    debug!("Command stdout: {}", stdout);
    debug!("Command stderr: {}", stderr);

    let code: Result<i32, DownloadError> = match cmd.status.code() {
        Some(code) => match code {
            0 => Ok(0),
            _ => Err(DownloadError::VideoExitErrorCode(code)),
        },
        None => Err(DownloadError::VideoExitNoCode),
    };
    code?;

    let tempfile = File::open(path)
        .await
        .map_err(|e| DownloadError::TempFileOpen(e))?;
    let stream = ReaderStream::new(tempfile);

    Ok(stream)
}
