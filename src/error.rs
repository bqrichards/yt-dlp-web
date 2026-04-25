use std::{
    io::{self},
    string::FromUtf8Error,
};

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
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
    #[error("UTF-8 conversion failed")]
    FromUtf8(#[source] FromUtf8Error),
}
