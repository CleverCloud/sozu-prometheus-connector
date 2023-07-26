//! # Logging module
//!
//! This module provides logging facilities and helpers

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use tracing::Level;
use tracing_subscriber::prelude::*;

// -----------------------------------------------------------------------------
// Error enumeration

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to set global default subscriber, {0}")]
    GlobalDefaultSubscriber(tracing::subscriber::SetGlobalDefaultError),
    #[error("failed to create a tracing registry")]
    TracingRegistry(tracing_subscriber::util::TryInitError),
}

// -----------------------------------------------------------------------------
// SentryContext

/// The URL and context used to forward errors to our sentry API
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct SentryContext {
    /// The Data Source Name of our API, typically "https://something@glitchtip.corp.clever-cloud.com/xx"
    pub dsn: String,
    /// Possible values: "development" or "production",
    /// see https://docs.rs/sentry/0.31.3/sentry/struct.ClientOptions.html#structfield.environment
    pub env: String,
}

impl SentryContext {
    pub fn new<T, S>(dsn: T, env: S) -> Self
    where
        T: ToString,
        S: ToString,
    {
        Self {
            dsn: dsn.to_string(),
            env: env.to_string(),
        }
    }
}

// -----------------------------------------------------------------------------
// LoggingInitGuard

#[derive(Default)]
pub struct LoggingInitGuard {
    #[allow(dead_code)]
    sentry_guard: Option<sentry::ClientInitGuard>,
}

impl From<Option<sentry::ClientInitGuard>> for LoggingInitGuard {
    #[tracing::instrument(skip_all)]
    fn from(sentry_guard: Option<sentry::ClientInitGuard>) -> Self {
        Self { sentry_guard }
    }
}

// -----------------------------------------------------------------------------
// Initialize logging system functions

/// Initialize the local logger
#[tracing::instrument]
pub fn initialize(verbosity: usize) -> Result<(), Error> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(level(verbosity))
            .with_thread_names(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .finish(),
    )
    .map_err(Error::GlobalDefaultSubscriber)
}

/// Initialize the local logger and the sentry hook.
///
/// Most importantly, keep the produced logging guard in a variable that will not
/// leave the main scope:
///
/// ```
/// use functions_sdk::logging::{initialize_with_sentry, SentryContext};
///
/// let _logging_guard_to_keep_around = initialize_with_sentry(
///     2,
///     SentryContext::new(
///         "https://something@glitchtip.corp.clever-cloud.com/xx",
///         "development",
///     )
/// ).expect("Could not initialize logging together with the sentry hook");
/// ```
#[tracing::instrument]
pub fn initialize_with_sentry(
    verbosity: usize,
    sentry_ctx: SentryContext,
) -> Result<LoggingInitGuard, Error> {
    let format_layer = tracing_subscriber::fmt::Layer::new()
        .with_writer(std::io::stdout.with_max_level(level(verbosity)))
        .with_thread_names(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true);

    let sentry_guard = LoggingInitGuard::from(sentry_initialize(sentry_ctx.dsn, sentry_ctx.env));

    tracing_subscriber::registry()
        .with(format_layer)
        .with(sentry_tracing::layer())
        .try_init()
        .map_err(Error::TracingRegistry)?;

    Ok(sentry_guard)
}

// -----------------------------------------------------------------------------
// helpers

pub const fn level(verbosity: usize) -> Level {
    match verbosity {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        _ => Level::TRACE,
    }
}

fn sentry_initialize(dsn: String, environment: String) -> Option<sentry::ClientInitGuard> {
    if dsn.is_empty() {
        return None;
    }

    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(Cow::from(environment)),
            ..sentry::ClientOptions::default()
        },
    )))
}
