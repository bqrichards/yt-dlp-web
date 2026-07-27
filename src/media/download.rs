use tracing::{debug, error, instrument};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::{
    error::DownloadError,
    media::{DownloadComplete, MediaOptions, cmd::DownloadMediaCommand},
};

#[instrument(skip_all, fields(%url))]
pub async fn download_media<F, Fut>(
    url: &str,
    media_options: MediaOptions,
    mut on_download_complete: F,
) -> Result<(), DownloadError>
where
    F: FnMut(DownloadComplete) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut cmd = DownloadMediaCommand::new(url, media_options);
    let cmd = cmd.command();

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
        if let Some(decoded) = DownloadMediaCommand::read_line(&line, media_options) {
            debug!("Calling on_download_complete with decoded: {:?}", decoded);
            on_download_complete(decoded).await;
        } else {
            // TODO the videos download in order of the titles so we know which video fails based on index.
            // For now we will discard, but this could be mapped to an error for the client.
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
