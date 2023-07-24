//! # Sozu pulsar connector
//!
//! This application retrieve requests from a pulsar topics and forward them to
//! Sōzu.

use std::{path::PathBuf, sync::Arc};

use clap::{ArgAction, Parser};
use tracing::{error, info};

use crate::svc::{
    config::{self, ConnectorConfiguration},
    http,
    logging::{self, LoggingInitGuard},
};

pub mod svc;

// -----------------------------------------------------------------------------
// Error

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to load configuration, {0}")]
    Configuration(config::Error),
    #[error("failed to initialize the logging system, {0}")]
    Logging(logging::Error),
    #[error("failed to create handler on termination signal, {0}")]
    Termination(std::io::Error),
    #[error("failed to serve http server, {0}")]
    HttpServer(http::server::Error),
    #[error("failed to load sōzu configuration, {0}")]
    SozuConfiguration(sozu_client::config::Error),
}

// -----------------------------------------------------------------------------
// Args

/// A connector to listen on the /metrics route,
/// request metrics from Sōzu that runs on the same machine
/// and return these metrics in a prometheus format
#[derive(Parser, PartialEq, Eq, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Increase verbosity
    #[clap(short = 'v', global = true, action = ArgAction::Count)]
    pub verbosity: u8,
    /// Path to the configuration file of the prometheus connector,
    #[clap(short = 'c', long = "config")]
    pub config: Option<PathBuf>,
}

impl paw::ParseArgs for Args {
    type Error = Error;

    fn parse_args() -> Result<Self, Self::Error> {
        Ok(Self::parse())
    }
}

// -----------------------------------------------------------------------------
// main

#[paw::main]
#[tokio::main(flavor = "current_thread")]
async fn main(args: Args) -> Result<(), Error> {
    // -------------------------------------------------------------------------
    // Retrieve configuration
    let config = Arc::new(match &args.config {
        Some(path) => {
            ConnectorConfiguration::try_from(path.to_owned()).map_err(Error::Configuration)?
        }
        None => ConnectorConfiguration::try_new().map_err(Error::Configuration)?,
    });

    // -------------------------------------------------------------------------
    // Initialize logging system
    let _guard = match &config.sentry {
        Some(sentry_ctx) => logging::initialize_with_sentry(args.verbosity  as usize, sentry_ctx.to_owned())
            .map_err(Error::Logging)?,
        None => logging::initialize(args.verbosity as usize)
            .map(|_| LoggingInitGuard::default())
            .map_err(Error::Logging)?,
    };

    // -------------------------------------------------------------------------
    // Load Sōzu configuration
    info!(
        path = config.sozu.configuration.display().to_string(),
        "Load Sōzu configuration"
    );
    let sozu_config = Arc::new(
        sozu_client::config::try_from(&config.sozu.configuration)
            .map_err(Error::SozuConfiguration)?,
    );

    // -------------------------------------------------------------------------
    // Start HTTP server and listener to termination signals concurrently and
    // not in parallel

    let result = tokio::select! {
        r = tokio::signal::ctrl_c() => r.map_err(Error::Termination),
        r = http::server::serve(config, sozu_config) => r.map_err(Error::HttpServer),
    };

    if let Err(err) = result {
        error!(
            error = err.to_string(),
            "Could not execute {} properly",
            env!("CARGO_PKG_NAME")
        );
        return Err(err);
    }

    info!("Gracefully halted {}!", env!("CARGO_PKG_NAME"));
    Ok(())
}
