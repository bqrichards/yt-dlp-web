use tracing::{debug, instrument};

use regex::Regex;
use tokio::process::Command;

use crate::error::DownloadError;

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
    let title_re = Regex::new(r".\[(\w+)\]\.mp4$").unwrap();
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
    let code: Result<i32, DownloadError> = match cmd.status.code() {
        Some(code) => match code {
            0 => Ok(0),
            _ => Err(DownloadError::TitleExitErrorCode(code)),
        },
        None => Err(DownloadError::TitleExitNoCode),
    };
    code?;

    let titles: Vec<VideoTitleId> = String::from_utf8(cmd.stdout)
        .map_err(|e| DownloadError::FromUtf8(e))?
        .lines()
        .filter_map(|l| {
            let mut video_id: Option<&str> = None;
            // TODO Handle `extract` panicing. we should fail gracefully instead.
            for (_, [id]) in title_re.captures_iter(l).map(|c| c.extract()) {
                video_id = Some(id);
            }

            video_id.map(|s| VideoTitleId {
                client_id: client_id.clone(),
                video_id: String::from(s),
                video_title: String::from(l),
            })
        })
        .collect();

    Ok(titles)
}
