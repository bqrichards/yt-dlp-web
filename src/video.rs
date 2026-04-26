use tempfile::env;
use tracing::{debug, instrument};

use tokio::process::Command;

use crate::error::DownloadError;

#[instrument]
pub async fn download_videos(url: &str) -> Result<(), DownloadError> {
    let mut path = env::temp_dir();
    let filename_template = "%(id)s.%(ext)s";
    path.push(filename_template);
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

    Ok(())
}
