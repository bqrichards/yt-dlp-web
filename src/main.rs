use std::net::SocketAddr;

use tracing::info;
use tracing_subscriber::{filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{port::Port, router::app_router};

mod delete_on_drop_stream;
mod endpoint;
mod error;
mod port;
mod router;
mod video;
mod ws;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(filter::LevelFilter::DEBUG)
        .with(fmt::layer())
        .init();

    let app = app_router();
    let port = Port::env(3000).get();
    let addr = format!("0.0.0.0:{}", port);

    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
