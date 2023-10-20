//! # Server module
//!
//! This module provide a server implementation with a lite router

use std::sync::Arc;

use axum::{
    middleware::{self},
    routing::{any, get},
    Router,
};
use hyper::Server;
use sozu_client::{channel::ConnectionProperties, config::canonicalize_command_socket, Client};
use sozu_command_lib::config::Config;
use tracing::{debug, info};

use crate::svc::config::ConnectorConfiguration;

pub mod handler;
pub mod layer;

// -----------------------------------------------------------------------------
// Error

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to bind server, {0}")]
    Bind(hyper::Error),
    #[error("failed to serve content, {0}")]
    Serve(hyper::Error),
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
}

impl From<Client> for State {
    #[tracing::instrument]
    fn from(client: Client) -> Self {
        Self { client }
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
    let state = State::from(client);

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
    // Serve router
    info!(
        addr = config.listening_address.to_string(),
        "Begin to listen on address"
    );

    Server::try_bind(&config.listening_address)
        .map_err(Error::Bind)?
        .serve(router.into_make_service())
        .await
        .map_err(Error::Serve)?;

    Ok(())
}
