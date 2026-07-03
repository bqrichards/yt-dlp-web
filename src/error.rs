use std::io::{self};

use tokio::task::JoinError;

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("failed to spawn video command")]
    CommandSpawn(#[source] io::Error),
    #[error("failed to await video command exit code")]
    CommandExitCode(#[source] io::Error),
    #[error("failed to read stdout of video command")]
    CommandStdoutOutput(#[source] io::Error),
    #[error("failed to read stderr of video command")]
    CommandStderrOutput(#[source] io::Error),
    #[error("failed to join on video command")]
    CommandJoin(#[source] JoinError),
    #[error("could not read stdout from video command")]
    CommandNoStdout,
    #[error("could not read stderr from video command")]
    CommandNoStderr,
    #[error("video download command exited with no status code")]
    ExitNoCode,
    #[error("video download command exited with status code {0}")]
    ExitErrorCode(i32),
}
