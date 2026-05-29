use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error};

use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;

use crate::{
    error::DownloadError,
    video::{self, DownloadComplete},
};

#[derive(Deserialize)]
struct ClientDownloadMessage {
    client_id: uuid::Uuid,
    url: String,
}

#[derive(Debug, Serialize)]
struct ServerVideoReadyMessage {
    message_type: String,
    client_id: uuid::Uuid,
    video_id: String,
    video_title: String,
    download_url: String,
}

#[derive(Serialize)]
struct ServerErrorMessage {
    message_type: String,
    error_message: String,
}

#[derive(Serialize)]
struct ServerVideoErrorMessage {
    message_type: String,
    error_message: String,
    video_id: String,
}

#[derive(Debug, Serialize)]
struct RequestFinishedMessage {
    message_type: String,
    client_id: uuid::Uuid,
    success: bool,
}

enum DownloadEvent {
    Video(DownloadComplete),
    Done(Option<DownloadError>),
}

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

    let cmd = match cmd {
        Some(cmd) => cmd,
        None => {
            debug!("{who} send unknown command");
            send_error(&mut socket, who, &ServerErrorMessage::bad_request()).await;
            return;
        }
    };

    let ClientDownloadMessage { client_id, url } = cmd;
    debug!("{who} = {} -> download {}", &client_id, &url);

    let (tx, mut rx) = mpsc::channel::<DownloadEvent>(32);

    let sender_task = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            match v {
                DownloadEvent::Video(v) => {
                    let video_ready = ServerVideoReadyMessage {
                        message_type: "video_ready".to_string(),
                        client_id,
                        video_id: v.id.clone(),
                        video_title: v.title.clone(),
                        download_url: format!("/api/download?id={}", v.id.clone()),
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
                DownloadEvent::Done(download_error) => {
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

    let download_videos_task = video::download_videos(&url, |v| {
        let tx = tx.clone();

        async move {
            if let Err(e) = tx.send(DownloadEvent::Video(v.clone())).await {
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
    if let Err(e) = tx.send(DownloadEvent::Done(download_videos_task_err)).await {
        error!("error sending download done event to client: {:?}", e)
    }

    if let Err(e) = sender_task.await {
        error!("sender_task join error: {:?}", e)
    }
}

impl ServerErrorMessage {
    fn bad_request() -> ServerErrorMessage {
        Self {
            message_type: "error".to_string(),
            error_message: "Client sent bad request".to_string(),
        }
    }
}

impl From<ServerVideoReadyMessage> for ServerVideoErrorMessage {
    fn from(value: ServerVideoReadyMessage) -> Self {
        Self {
            message_type: "error".to_string(),
            error_message: "Internal Server Error".to_string(),
            video_id: value.video_id,
        }
    }
}

async fn send_error<T>(socket: &mut WebSocket, who: SocketAddr, error: &T)
where
    T: ?Sized + Serialize,
{
    let message = match serde_json::to_string(error) {
        Ok(message) => message,
        Err(e) => {
            error!("Error serializing error message: {e}");
            return;
        }
    };

    if socket.send(Message::Text(message.into())).await.is_err() {
        debug!("client {who} abruptly disconnected");
    }
}

/// Parse message into request.
fn process_message(
    msg: Message,
    who: SocketAddr,
) -> ControlFlow<(), Option<ClientDownloadMessage>> {
    match msg {
        Message::Text(t) => {
            debug!(">>> {who} sent str: {t:?}");
            let cmd: Result<ClientDownloadMessage, serde_json::Error> = serde_json::from_str(&t);
            ControlFlow::Continue(cmd.ok())
        }
        Message::Binary(d) => {
            debug!(">>> {who} sent {} bytes: {d:?}", d.len());
            ControlFlow::Continue(None)
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                debug!(
                    ">>> {who} sent close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                debug!(">>> {who} somehow sent close message without CloseFrame");
            }
            ControlFlow::Break(())
        }

        Message::Pong(v) => {
            debug!(">>> {who} sent pong with {v:?}");
            ControlFlow::Continue(None)
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            debug!(">>> {who} sent ping with {v:?}");
            ControlFlow::Continue(None)
        }
    }
}
