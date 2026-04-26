use tracing::{debug, instrument};

use regex::Regex;
use tokio::process::Command;

use crate::error::DownloadError;

#[derive(Debug)]
pub struct VideoTitleId {
    pub client_id: uuid::Uuid,
    pub video_id: String,
    pub video_title: String,
}

#[instrument]
pub async fn get_video_titles(
    client_id: &uuid::Uuid,
    url: &str,
) -> Result<Vec<VideoTitleId>, DownloadError> {
    let filename_template = "%(uploader)s - %(title)s [%(id)s].%(ext)s";
    let cmd = Command::new("yt-dlp")
        .arg("-S")
        .arg("res,ext:mp4:m4a")
        .arg("--recode")
        .arg("mp4")
        .arg("--print")
        .arg("filename")
        .arg("-o")
        .arg(&filename_template)
        .arg(url)
        .output()
        .await
        .map_err(|e| DownloadError::TitleCommand(e))?;

    debug!("Command status: {}", cmd.status);
    let stdout = String::from_utf8(cmd.stdout).map_err(|e| DownloadError::FromUtf8(e))?;
    let stderr = String::from_utf8(cmd.stderr).map_err(|e| DownloadError::FromUtf8(e))?;
    debug!("Command stdout: {}", stdout);
    debug!("Command stderr: {}", stderr);

    let code: Result<i32, DownloadError> = match cmd.status.code() {
        Some(code) => match code {
            0 => Ok(0),
            _ => Err(DownloadError::TitleExitErrorCode(code)),
        },
        None => Err(DownloadError::TitleExitNoCode),
    };
    code?;

    let title_re = Regex::new(r".\[([A-Za-z0-9_-]+)\]\.mp4$").unwrap();
    let titles: Vec<VideoTitleId> = stdout
        .lines()
        .filter_map(|l| {
            let video_id = title_re
                .captures_iter(l)
                .find_map(|caps| caps.get(1).map(|m| m.as_str()));

            video_id.map(|s| VideoTitleId {
                client_id: client_id.clone(),
                video_id: String::from(s),
                video_title: String::from(l),
            })
        })
        .collect();
    debug!("titles: {:?}", titles);

    Ok(titles)
}
