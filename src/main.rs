pub mod metrics_server;
pub mod sozu_channel;

use crate::metrics_server::{get_metrics, metrics_app};
use anyhow::Context;
use axum::{routing::get, Router};
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let address: std::net::SocketAddr = "127.0.0.1:3000"
        .parse()
        .with_context(|| "Could not parse listening address")?;

    info!("Starting listening on {}/metrics", address.to_string());

    let metrics_app = Router::new().route("/metrics", get(get_metrics));

    axum::Server::bind(&address)
        .serve(metrics_app.into_make_service())
        .await
        .with_context(|| "axum server crashed")
}
