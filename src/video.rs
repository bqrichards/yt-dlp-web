use std::process::Stdio;

use tempfile::env;
use tracing::{debug, error, instrument};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
};

use crate::error::DownloadError;

#[derive(Debug, Clone)]
pub struct DownloadComplete {
    pub id: String,
    pub title: String,
    pub media_options: MediaOptions,
}

#[instrument(skip_all, fields(%url))]
pub async fn download_videos<F, Fut>(
    url: &str,
    media_options: MediaOptions,
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
    change_download(&media_options, &mut cmd);
    // FIXME We now have a bug where if a video is downloaded with a certain max_resolution,
    // changing that max resolution and downloading again will return the old video because
    // the filename is only the id and title, does not include resolution.
    cmd.arg("--newline")
        .arg("--print")
        .arg(format!("after_move:%(id)s{}%(title)s", DELIM))
        .arg("-o")
        .arg(&path)
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(DownloadError::CommandSpawn)?;

    // Pipe stdout from command so we can send video to client when download is complete
    let stdout = child.stdout.take().ok_or(DownloadError::CommandNoStdout)?;
    let mut reader = BufReader::new(stdout).lines();

    // Pipe stderr from command so we can send log any errors
    let stderr = child.stderr.take().ok_or(DownloadError::CommandNoStderr)?;
    let mut reader_err = BufReader::new(stderr);

    let child_task = tokio::spawn(async move { child.wait().await });

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(DownloadError::CommandStdoutOutput)?
    {
        // TODO the videos download in order of the titles so we know which video fails based on index.
        // For now we will discard, but this could be mapped to an error for the client.
        if let Some((id, title)) = line.split_once(DELIM) {
            let decoded = DownloadComplete {
                id: id.to_string(),
                title: title.to_string(),
                media_options,
            };
            debug!("Calling on_download_complete with decoded: {:?}", decoded);
            on_download_complete(decoded).await;
        } else {
            error!("video download could not be decoded from line: {:?}", line);
        }
    }

    // Read stderr into a String
    let mut stderr_output = String::new();
    let stderr_size = reader_err
        .read_to_string(&mut stderr_output)
        .await
        .map_err(DownloadError::CommandStderrOutput)?;
    if stderr_size > 0 {
        error!(stderr_output);
    }

    // Wait for command to finish
    let exit_status = child_task
        .await
        .map_err(DownloadError::CommandJoin)?
        .map_err(DownloadError::CommandExitCode)?;

    match exit_status.code() {
        Some(code) => match code {
            0 => Ok(()),
            _ => Err(DownloadError::ExitErrorCode(code)),
        },
        None => Err(DownloadError::ExitNoCode),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VideoResolution {
    /// 1080p
    FHD,
    /// 1440p (2K)
    QHD,
    /// 2160p (4K)
    UHD,
}

impl VideoResolution {
    pub fn height(&self) -> i32 {
        match self {
            Self::FHD => 1080,
            Self::QHD => 1440,
            Self::UHD => 2160,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaOptions {
    Audio,
    Video { max_resolution: VideoResolution },
}

fn change_download(download_options: &MediaOptions, cmd: &mut Command) {
    match download_options {
        MediaOptions::Audio => {
            cmd.arg("-t").arg("mp3");
        }
        MediaOptions::Video { max_resolution } => {
            let resolution = max_resolution.height();
            let format_cmd =
                format!("bv[height<={resolution}][vcodec^=avc1]+ba/best[height<={resolution}]");
            cmd.arg("-f")
                .arg(format_cmd)
                .arg("--merge-output-format")
                .arg("mp4");
        }
    }
}
