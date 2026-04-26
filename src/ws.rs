use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;

use crate::{
    title::{self, VideoTitleId},
    video,
};

#[derive(Deserialize)]
struct ClientDownloadMessage {
    client_id: uuid::Uuid,
    url: String,
}

#[derive(Serialize)]
struct ServerVideoManifestMessage {
    message_type: String,
    video_count: usize,
    // TODO The elements in this array do have "message_type": "video_ready" which isn't really
    // correct but is also not an implementation issue, just semantic. Fix later.
    videos: Vec<ServerVideoReadyMessage>,
}

impl From<&Vec<VideoTitleId>> for ServerVideoManifestMessage {
    fn from(value: &Vec<VideoTitleId>) -> Self {
        Self {
            message_type: "manifest".to_string(),
            video_count: value.len(),
            videos: value.iter().map(|f| f.into()).collect(),
        }
    }
}

#[derive(Serialize)]
struct ServerVideoReadyMessage {
    message_type: String,
    client_id: uuid::Uuid,
    video_id: String,
    video_title: String,
    download_url: String,
}

impl From<&VideoTitleId> for ServerVideoReadyMessage {
    fn from(value: &VideoTitleId) -> Self {
        Self {
            message_type: "video_ready".to_string(),
            client_id: value.client_id.clone(),
            video_id: value.video_id.clone(),
            video_title: value.video_title.clone(),
            download_url: format!("/api/download?id={}", value.video_id.clone()),
        }
    }
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
            // TODO return message to client
            debug!("{who} send unknown command");
            return;
        }
    };

    let ClientDownloadMessage { client_id, url } = cmd;
    debug!("{who} = {} -> download {}", &client_id, &url);

    let video_titles = title::get_video_titles(&client_id, &url).await;
    let titles = match video_titles {
        Ok(titles) => titles,
        Err(e) => {
            // TODO send error to client
            error!("error getting video titles: {}", e);
            return;
        }
    };

    let manifest: ServerVideoManifestMessage = (&titles).into();
    let manifest_string = match serde_json::to_string(&manifest) {
        Ok(manifest_msg) => manifest_msg,
        Err(e) => {
            // TODO send error to client
            error!("could not serialize message: {}", e);
            return;
        }
    };
    if socket
        .send(Message::Text(manifest_string.into()))
        .await
        .is_err()
    {
        debug!("client {who} abruptly disconnected");
        return;
    }

    // TODO Send back the videos as they are ready so they can be downloaded ASAP.
    // TODO Set exp date for each file so we can cleanup later.
    let _ = video::download_videos(&url).await;

    for title in titles {
        let video_ready: ServerVideoReadyMessage = (&title).into();
        let video_ready_message = serde_json::to_string(&video_ready);
        match video_ready_message {
            Ok(d) => {
                if socket.send(Message::Text(d.into())).await.is_err() {
                    debug!("client {who} abruptly disconnected");
                    return;
                }
            }
            Err(e) => {
                // TODO send error to client
                error!("could not serialize message: {}", e);
                return;
            }
        }
    }
}

/// Read message as request.
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
