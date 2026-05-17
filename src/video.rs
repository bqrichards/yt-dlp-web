use std::process::Stdio;

use tempfile::env;
use tracing::{debug, error, instrument};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::error::DownloadError;

#[derive(Debug, Clone)]
pub struct DownloadComplete {
    pub id: String,
    pub title: String,
}

#[instrument(skip_all, fields(%url))]
pub async fn download_videos<F, Fut>(
    url: &str,
    mut on_download_complete: F,
) -> Result<(), DownloadError>
where
    F: FnMut(DownloadComplete) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut path = env::temp_dir();
    let filename_template = "%(id)s.%(ext)s";
    path.push(filename_template);
    debug!("Temp File Path: {:?}", path);

    /// Delimiter between video id and video title
    const DELIM: char = '\x1F';
    let mut cmd = Command::new("yt-dlp");
    cmd.arg("-S")
        .arg("res,ext:mp4:m4a")
        .arg("--recode")
        .arg("mp4")
        .arg("--newline")
        .arg("--print")
        .arg(format!("after_move:%(id)s{}%(title)s", DELIM))
        .arg("-o")
        .arg(&path)
        .arg(url)
        .stdout(Stdio::piped());

    // Pipe stdout from command so we can send video to client when download is complete
    let mut child = cmd.spawn().map_err(DownloadError::Command)?;
    let stdout = child.stdout.take().ok_or(DownloadError::CommandNoStdout)?;

    let mut reader = BufReader::new(stdout).lines();
    let child_task = tokio::spawn(async move { child.wait().await });

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(DownloadError::CommandOutput)?
    {
        // TODO the videos download in order of the titles so we know which video fails based on index.
        // For now we will discard, but this could be mapped to an error for the client.
        if let Some((id, title)) = line.split_once(DELIM) {
            let decoded = DownloadComplete {
                id: id.to_string(),
                title: title.to_string(),
            };
            debug!("Calling on_download_complete with decoded: {:?}", decoded);
            on_download_complete(decoded).await;
        } else {
            error!("video download could not be decoded from line: {:?}", line);
        }
    }

    // Wait for command to finish
    let exit_status = child_task
        .await
        .map_err(DownloadError::CommandJoin)?
        .map_err(DownloadError::Command)?;

    match exit_status.code() {
        Some(code) => match code {
            0 => Ok(()),
            _ => Err(DownloadError::ExitErrorCode(code)),
        },
        None => Err(DownloadError::ExitNoCode),
    }
}
