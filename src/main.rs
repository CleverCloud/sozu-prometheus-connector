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

    let metrics_path = "/metrics";

    let app = Router::new().route(metrics_path, get(|| async { "Metrics will come soon" }));

    info!(
        "Starting listening on {}{}",
        address.to_string(),
        metrics_path
    );

    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .with_context(|| "axum server crashed")
}
