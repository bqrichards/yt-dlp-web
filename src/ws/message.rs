use std::{net::SocketAddr, ops::ControlFlow};

use axum::extract::ws::Message;
use tracing::{debug, error};

use crate::ws::ClientStartDownloadMessage;

/// Parse message into request.
pub fn process_message(
    msg: Message,
    who: SocketAddr,
) -> ControlFlow<(), Option<ClientStartDownloadMessage>> {
    match msg {
        Message::Text(t) => {
            debug!(">>> {who} sent str: {t:?}");
            let cmd: Result<ClientStartDownloadMessage, serde_json::Error> =
                serde_json::from_str(&t);
            match cmd {
                Ok(cmd) => ControlFlow::Continue(Some(cmd)),
                Err(err) => {
                    error!("error decoding message as ServerStartDownloadMessage: {t}: {err}");
                    ControlFlow::Continue(None)
                }
            }
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
