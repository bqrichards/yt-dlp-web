use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tempfile::env;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument};

use axum::{
    body::Body,
    extract::Query,
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
};

use crate::{delete_on_drop_stream::DeleteOnDropStream, ws::MediaFormat};

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoObject {
    id: String,
    media_format: MediaFormat,
}

static VIDEO_ID_RE: OnceLock<Regex> = OnceLock::new();

fn video_id_re() -> &'static Regex {
    VIDEO_ID_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{1,64}$").unwrap())
}

#[instrument]
pub async fn download_video_endpoint(
    Query(payload): Query<VideoObject>,
) -> Result<Response<Body>, Response<Body>> {
    let VideoObject { id, media_format } = payload;
    if !video_id_re().is_match(&id) {
        return Err((StatusCode::BAD_REQUEST, "invalid video id").into_response());
    }
    let ext = media_format.ext();

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
    let stream = DeleteOnDropStream::new(stream, path.clone());
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(media_format.content_type()).unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str("attachment").unwrap(),
    );

    debug!(headers = ?headers, "Headers");

    Ok((headers, body).into_response())
}
