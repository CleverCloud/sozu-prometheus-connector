pub mod metrics_server;

use crate::metrics_server::metrics_app;
use anyhow::Context;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let address: std::net::SocketAddr = "127.0.0.1:3000"
        .parse()
        .with_context(|| "Could not parse listening address")?;

    // let sozu_socket_path = "/home/emmanuel/clever/sozu_for_the_win/github_repo/bin/sozu.sock";

    info!("Starting listening on {}/metrics", address.to_string());

    let metrics_app = metrics_app();

    axum::Server::bind(&address)
        .serve(metrics_app.into_make_service())
        .await
        .with_context(|| "axum server crashed")
}
