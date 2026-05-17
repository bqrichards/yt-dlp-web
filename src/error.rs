use std::io::{self};

use tokio::task::JoinError;

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to run video command")]
    Command(#[source] io::Error),
    #[error("failed to read output of video command")]
    CommandOutput(#[source] io::Error),
    #[error("failed to join on video command")]
    CommandJoin(#[source] JoinError),
    #[error("could not read stdout from video command")]
    CommandNoStdout,
    #[error("video download command exited with no status code")]
    ExitNoCode,
    #[error("video download command exited with status code {0}")]
    ExitErrorCode(i32),
}
