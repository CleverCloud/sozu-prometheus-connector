use anyhow::{bail, Context};
use axum::{http::StatusCode, routing::get, Router};
use tracing::{error, info};

use sozu_command_lib::{
    channel::Channel,
    proto::command::{
        request::RequestType, QueryMetricsOptions, Request, Response, ResponseStatus,
    },
};

// todo: replace with sozu_command_lib::config default values when bumping the dependency
const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1_000_000;
const DEFAULT_MAX_COMMAND_BUFFER_SIZE: usize = 2_000_000;
// TODO: replace this path with some env variable or config or something
const SOZU_SOCKET_PATH: &str = "/home/emmanuel/clever/sozu_for_the_win/github_repo/bin/sozu.sock";

pub fn metrics_app() -> Router {
    Router::new().route("/metrics", get(get_metrics))
}

pub async fn get_metrics() -> Result<String, StatusCode> {
    write_to_sozu_read_from_sozu().await.map_err(|sozu_error| {
        error!(
            "Could not write the metrics request to sozu or receive a response: {:#}",
            sozu_error
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub async fn write_to_sozu_read_from_sozu() -> anyhow::Result<String> {
    info!("Got GET request for sozu metrics");
    let mut sozu_channel: Channel<Request, Response> = Channel::from_path(
        SOZU_SOCKET_PATH,
        DEFAULT_COMMAND_BUFFER_SIZE,
        DEFAULT_MAX_COMMAND_BUFFER_SIZE,
    )
    .with_context(|| {
        format!(
            "Could not create a sozu channel from path {}",
            SOZU_SOCKET_PATH
        )
    })?;

    sozu_channel
        .blocking()
        .with_context(|| "Could not block the sozu channel")?;

    let metrics_request = Request {
        request_type: Some(RequestType::QueryMetrics(QueryMetricsOptions::default())),
    };

    info!(
        "Created a Sōzu request for all metrics: {:?}",
        metrics_request
    );

    sozu_channel
        .write_message(&metrics_request)
        .with_context(|| "Could not write metrics request on the sozu channel")?;

    loop {
        let response = sozu_channel
            .read_message()
            .with_context(|| "failed to read message on the sozu channel ")?;
        match response.status() {
            ResponseStatus::Processing => info!("Sozu is processing…"),
            ResponseStatus::Failure => bail!(response.message),
            ResponseStatus::Ok => return Ok(format!("{:?}", response.content)),
        }
    }
}
