use std::{
    io::{self},
    string::FromUtf8Error,
};

use tokio::task::JoinError;

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to run title command")]
    TitleCommand(#[source] io::Error),
    #[error("failed to run video command")]
    VideoCommand(#[source] io::Error),
    #[error("failed to read output of video command")]
    VideoCommandOutput(#[source] io::Error),
    #[error("failed to join on video command")]
    VideoCommandJoin(#[source] JoinError),
    #[error("could not read stdout from video command")]
    VideoCommandNoStdout,
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
