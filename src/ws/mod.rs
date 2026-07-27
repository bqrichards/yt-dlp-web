mod handler;
mod message;

pub use handler::*;

use axum::extract::ws::{Message, WebSocket};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use std::net::SocketAddr;

use crate::media::{self, MediaOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaFormat {
    Audio,
    Video,
}

impl MediaFormat {
    /// Extension of the file format. Does not include '.' prefix.
    pub fn ext(&self) -> &str {
        match self {
            Self::Audio => "mp3",
            Self::Video => "mp4",
        }
    }

    /// MIME type of the format.
    pub fn content_type(&self) -> &str {
        match self {
            Self::Audio => "audio/mpeg",
            Self::Video => "video/mp4",
        }
    }
}

impl std::fmt::Display for MediaFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Audio => "Audio",
            Self::Video => "Video",
        })
    }
}

#[derive(Debug, Deserialize)]
pub enum VideoResolution {
    /// 1080p
    Fhd,
    /// 1440p (2K)
    Qhd,
    /// 2160p (4K)
    Uhd,
}

#[derive(Debug)]
pub enum ParseDownloadFormError {
    MissingResolution,
}

impl From<VideoResolution> for media::VideoResolution {
    fn from(value: VideoResolution) -> Self {
        match value {
            VideoResolution::Fhd => Self::Fhd,
            VideoResolution::Qhd => Self::Qhd,
            VideoResolution::Uhd => Self::Uhd,
        }
    }
}

impl From<MediaOptions> for MediaFormat {
    fn from(value: MediaOptions) -> Self {
        match value {
            MediaOptions::Audio => Self::Audio,
            MediaOptions::Video { .. } => Self::Video,
        }
    }
}

impl TryFrom<(MediaFormat, Option<VideoResolution>)> for MediaOptions {
    type Error = ParseDownloadFormError;

    fn try_from(value: (MediaFormat, Option<VideoResolution>)) -> Result<Self, Self::Error> {
        let (media_format, video_resolution) = value;
        match media_format {
            MediaFormat::Audio => Ok(Self::Audio),
            MediaFormat::Video => Ok(Self::Video {
                max_resolution: video_resolution
                    .ok_or(ParseDownloadFormError::MissingResolution)?
                    .into(),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ClientStartDownloadMessage {
    client_id: uuid::Uuid,
    url: String,
    media_format: MediaFormat,
    video_resolution: Option<VideoResolution>,
}

#[derive(Debug, Serialize)]
pub struct ServerVideoReadyMessage {
    message_type: String,
    client_id: uuid::Uuid,
    video_id: String,
    video_title: String,
    media_format: MediaFormat,
    download_url: String,
}

#[derive(Serialize)]
pub struct ServerErrorMessage {
    message_type: String,
    error_message: String,
}

#[derive(Serialize)]
pub struct ServerVideoErrorMessage {
    message_type: String,
    error_message: String,
    video_id: String,
}

#[derive(Debug, Serialize)]
pub struct RequestFinishedMessage {
    message_type: String,
    client_id: uuid::Uuid,
    success: bool,
}

impl ServerErrorMessage {
    pub fn bad_request() -> ServerErrorMessage {
        Self {
            message_type: "error".to_string(),
            error_message: "Client sent bad request".to_string(),
        }
    }
}

impl From<ServerVideoReadyMessage> for ServerVideoErrorMessage {
    fn from(value: ServerVideoReadyMessage) -> Self {
        Self {
            message_type: "error".to_string(),
            error_message: "Internal Server Error".to_string(),
            video_id: value.video_id,
        }
    }
}

async fn send_error<T>(socket: &mut WebSocket, who: SocketAddr, error: &T)
where
    T: ?Sized + Serialize,
{
    let message = match serde_json::to_string(error) {
        Ok(message) => message,
        Err(e) => {
            error!("Error serializing error message: {e}");
            return;
        }
    };

    debug!("sending message through socket: {:?}", message);
    if socket.send(Message::Text(message.into())).await.is_err() {
        debug!("client {who} abruptly disconnected");
    }
}
