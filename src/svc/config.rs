//! # Configuration module
//!
//! This module provides structures and helpers to interact with the configuration

use std::{
    env::{self, VarError},
    net::SocketAddr,
    path::PathBuf,
};

use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

use crate::svc::logging::SentryContext;

// -----------------------------------------------------------------------------
// Error

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to build configuration, {0}")]
    Build(ConfigError),
    #[error("failed to serialize configuration, {0}")]
    Serialize(ConfigError),
    #[error("failed to retrieve environment variable '{0}', {1}")]
    EnvironmentVariable(&'static str, VarError),
}

// -----------------------------------------------------------------------------
// Sozu

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Sozu {
    #[serde(rename = "configuration")]
    pub configuration: PathBuf,
}

// -----------------------------------------------------------------------------
// Configuration

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ConnectorConfiguration {
    #[serde(rename = "listening-address")]
    pub listening_address: SocketAddr,
    #[serde(rename = "sozu")]
    pub sozu: Sozu,
    #[serde(rename = "sentry")]
    pub sentry: Option<SentryContext>,
}

impl TryFrom<PathBuf> for ConnectorConfiguration {
    type Error = Error;

    #[tracing::instrument]
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Config::builder()
            .add_source(File::from(path).required(true))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Serialize)
    }
}

impl ConnectorConfiguration {
    #[tracing::instrument]
    pub fn try_new() -> Result<Self, Error> {
        let homedir = env::var("HOME").map_err(|err| Error::EnvironmentVariable("HOME", err))?;

        Config::builder()
            .add_source(
                File::from(PathBuf::from(format!(
                    "/usr/share/{}/config",
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "/etc/{}/config",
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "{}/.config/{}/config",
                    homedir,
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(
                File::from(PathBuf::from(format!(
                    "{}/.local/share/{}/config",
                    homedir,
                    env!("CARGO_PKG_NAME")
                )))
                .required(false),
            )
            .add_source(File::from(PathBuf::from("config")).required(false))
            .build()
            .map_err(Error::Build)?
            .try_deserialize()
            .map_err(Error::Serialize)
    }
}
