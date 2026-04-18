use tracing::{debug, instrument};

use tokio::process::Command;

use crate::error::DownloadError;

#[instrument]
pub async fn get_video_titles(url: &str) -> Result<Vec<String>, DownloadError> {
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

    let titles = String::from_utf8(cmd.stdout)
        .map_err(|e| DownloadError::FromUtf8(e))?
        .lines()
        .map(|l| String::from(l))
        .collect();

    Ok(titles)
}
