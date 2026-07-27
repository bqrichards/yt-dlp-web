use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use tokio::sync::mpsc;
use tracing::{debug, error};

use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;

use crate::{
    error::DownloadError,
    media::{self, DownloadComplete, MediaOptions},
    ws::{
        ClientStartDownloadMessage, MediaFormat, RequestFinishedMessage, ServerErrorMessage,
        ServerVideoErrorMessage, ServerVideoReadyMessage, message::process_message, send_error,
    },
};

/// The handler for the HTTP request (this gets called when the HTTP request lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    debug!("{addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    let msg = match socket.recv().await {
        Some(msg) => match msg {
            Ok(msg) => msg,
            Err(e) => {
                error!("error reading msg from client {who}: {e}");
                return;
            }
        },
        None => {
            debug!("client {who} abruptly disconnected");
            return;
        }
    };

    let ControlFlow::Continue(cmd) = process_message(msg, who) else {
        debug!("message break");
        return;
    };

    let Some(cmd) = cmd else {
        debug!("{who} sent an unknown command");
        send_error(&mut socket, who, &ServerErrorMessage::bad_request()).await;
        return;
    };

    let ClientStartDownloadMessage {
        client_id,
        url,
        media_format,
        video_resolution,
    } = cmd;
    debug!("{who} = {} -> download {}", &client_id, &url);

    let Ok(media_options) = MediaOptions::try_from((media_format, video_resolution)) else {
        debug!("could not parse media options from server download message");
        send_error(&mut socket, who, &ServerErrorMessage::bad_request()).await;
        return;
    };

    let (tx, mut rx) = mpsc::channel::<ControlFlow<Option<DownloadError>, DownloadComplete>>(32);

    let sender_task = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            match v {
                ControlFlow::Continue(v) => {
                    let media_format: MediaFormat = v.media_options.into();
                    let video_ready = ServerVideoReadyMessage {
                        message_type: "video_ready".to_string(),
                        client_id,
                        video_id: v.id.clone(),
                        video_title: v.title.clone(),
                        media_format: media_format.clone(),
                        download_url: format!(
                            "/api/download?id={}&media_format={}",
                            v.id.clone(),
                            media_format,
                        ),
                    };
                    debug!("video_ready message: {:?}", video_ready);
                    let video_ready_message = match serde_json::to_string(&video_ready) {
                        Ok(v) => v,
                        Err(e) => {
                            error!("server could not serialize video ready message: {e}");
                            send_error(
                                &mut socket,
                                who,
                                &ServerVideoErrorMessage::from(video_ready),
                            )
                            .await;
                            return;
                        }
                    };

                    debug!("sending message through socket: {:?}", video_ready_message);
                    if socket
                        .send(Message::Text(video_ready_message.into()))
                        .await
                        .is_err()
                    {
                        debug!("client {who} abruptly disconnected");
                        return;
                    }
                }
                ControlFlow::Break(download_error) => {
                    // Tell client all videos are finished
                    let request_finished = RequestFinishedMessage {
                        message_type: "request_finished".to_string(),
                        client_id,
                        success: download_error.is_none(),
                    };
                    debug!("request_finished message: {:?}", request_finished);
                    let request_finished_message = match serde_json::to_string(&request_finished) {
                        Ok(v) => v,
                        Err(e) => {
                            error!("server could not serialize request_finished_message: {e}");
                            return;
                        }
                    };

                    debug!(
                        "sending message through socket: {:?}",
                        request_finished_message
                    );
                    if socket
                        .send(Message::Text(request_finished_message.into()))
                        .await
                        .is_err()
                    {
                        debug!("client {who} abruptly disconnected");
                    }

                    break;
                }
            }
        }
    });

    let download_videos_task = media::download_media(&url, media_options, |v| {
        let tx = tx.clone();

        async move {
            if let Err(e) = tx.send(ControlFlow::Continue(v.clone())).await {
                error!("tx error: {:?}", e)
            }
        }
    })
    .await;

    let download_videos_task_err = download_videos_task.err();
    if let Some(e) = &download_videos_task_err {
        error!("error downloading video: {:?}", e)
    }

    // downloading videos is done. send done message in channel.
    if let Err(e) = tx.send(ControlFlow::Break(download_videos_task_err)).await {
        error!("error sending download done event to client: {:?}", e)
    }

    if let Err(e) = sender_task.await {
        error!("sender_task join error: {:?}", e)
    }
}
