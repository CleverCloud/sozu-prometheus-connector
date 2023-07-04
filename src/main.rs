pub mod cli;
pub mod config;
pub mod metrics_server;
pub mod prometheus;
pub mod sozu_channel;

use crate::{
    config::{get_socket_path_from_sozu_config, parse_connector_config_file_for_sozu_config_path},
    metrics_server::get_metrics,
    sozu_channel::initialize_sozu_channel,
};
use anyhow::Context;
use axum::{routing::get, Router};
use clap::Parser;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = cli::Args::parse();
    info!(
        "configuration file for the prometheus connector: {:?}",
        args.config,
    );

    let sozu_configuration_path = parse_connector_config_file_for_sozu_config_path(&args.config)?;

    info!("Loading Sōzu configuration");
    let sozu_socket_path = get_socket_path_from_sozu_config(sozu_configuration_path)
        .with_context(|| "Could not load sozu config")?;

    info!(
        "Initializing channel to sozu socket at path {:?}",
        sozu_socket_path
    );
    initialize_sozu_channel(&sozu_socket_path)
        .await
        .with_context(|| "Could not initialize a channel to sozu. Check that the proxy is up.")?;

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
