//! # Server module
//!
//! This module provides a server implementation with a router based on the
//! crate [`axum`].

use std::{net::SocketAddr, sync::Arc};

use axum::{
    middleware,
    routing::{any, get},
    Router,
};
use sozu_client::{channel::ConnectionProperties, config::canonicalize_command_socket, Client};
use sozu_command_lib::config::Config;
use tokio::net::TcpListener;
use tracing::{debug, info};

use crate::svc::config::ConnectorConfiguration;

// -----------------------------------------------------------------------------
// Export module

pub mod handler;
pub mod layer;

// -----------------------------------------------------------------------------
// Error

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to bind on socket '{0}', {1}")]
    Bind(SocketAddr, std::io::Error),
    #[error("failed to listen on socket '{0}', {1}")]
    Serve(SocketAddr, std::io::Error),
    #[error("failed to create client, {0}")]
    CreateClient(sozu_client::Error),
    #[error("failed to canonicalize path to command socket, {0}")]
    CanonicalizeSocket(sozu_client::config::Error),
}

// -----------------------------------------------------------------------------
// State

#[derive(Clone, Debug)]
pub struct State {
    pub client: Client,
    pub config: Arc<ConnectorConfiguration>,
}

impl State {
    fn new(client: Client, config: Arc<ConnectorConfiguration>) -> Self {
        Self { client, config }
    }
}

// -----------------------------------------------------------------------------
// helpers

#[tracing::instrument(skip_all)]
pub async fn serve(
    config: Arc<ConnectorConfiguration>,
    sozu_config: Arc<Config>,
) -> Result<(), Error> {
    // -------------------------------------------------------------------------
    // Create client and state
    info!("Create Sōzu client");
    let mut opts = ConnectionProperties::from(&*sozu_config);
    if opts.socket.is_relative() {
        opts.socket = canonicalize_command_socket(&config.sozu.configuration, &sozu_config)
            .map_err(Error::CanonicalizeSocket)?;
    }

    debug!("Sōzu command socket is {:?}", opts.socket);
    let client = Client::try_new(opts).await.map_err(Error::CreateClient)?;
    let state = State::new(client, config.to_owned());

    // -------------------------------------------------------------------------
    // Create router
    let router = Router::new()
        .route("/healthz", get(handler::healthz))
        .route("/livez", get(handler::healthz))
        .route("/readyz", get(handler::healthz))
        .route("/status", get(handler::healthz))
        .route("/metrics", get(handler::telemetry))
        .with_state(state)
        .fallback(any(handler::not_found))
        .layer(middleware::from_fn(layer::access));

    // -------------------------------------------------------------------------
    // Bind to listener and serve content
    let listener = TcpListener::bind(&config.listening_address)
        .await
        .map_err(|err| Error::Bind(config.listening_address.to_owned(), err))?;

    info!(
        addr = config.listening_address.to_string(),
        "Begin to listen on address"
    );
    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|err| Error::Serve(config.listening_address, err))?;

    Ok(())
}
