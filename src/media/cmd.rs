use std::{env, process::Stdio};

use tokio::process::Command;
use tracing::debug;

use crate::media::{DownloadComplete, MediaOptions};

pub struct DownloadMediaCommand {
    cmd: Command,
}

impl DownloadMediaCommand {
    /// Delimiter between video id and video title
    const DELIM: char = '\x1F';

    pub fn new(url: &str, options: MediaOptions) -> Self {
        let mut path = env::temp_dir();
        let filename_template = "%(id)s.%(ext)s";
        path.push(filename_template);
        debug!("Temp File Path: {:?}", path);

        let mut cmd = Command::new("yt-dlp");
        change_download_cmd(&mut cmd, &options);
        cmd.arg("--newline")
            .arg("--print")
            .arg(format!(
                "after_move:%(id)s{}%(title)s",
                DownloadMediaCommand::DELIM
            ))
            .arg("-o")
            .arg(&path)
            .arg(url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        Self { cmd }
    }

    pub fn command(&mut self) -> &mut Command {
        &mut self.cmd
    }

    pub fn read_line(line: &str, media_options: MediaOptions) -> Option<DownloadComplete> {
        line.split_once(DownloadMediaCommand::DELIM)
            .map(|(id, title)| DownloadComplete {
                id: id.to_string(),
                title: title.to_string(),
                media_options,
            })
    }
}

fn change_download_cmd(cmd: &mut Command, options: &MediaOptions) {
    match options {
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
