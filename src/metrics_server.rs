use anyhow::Context;
use axum::{http::StatusCode, routing::get, Router};
use tracing::{error, info};

use crate::sozu_channel::{new_sozu_channel, SOZU_CHANNEL};

pub fn metrics_app() -> Router {
    Router::new().route("/metrics", get(get_metrics))
}

pub async fn get_metrics() -> Result<String, StatusCode> {
    SOZU_CHANNEL.with(|channel| {
        let mut channel_resurrection_retries = 0usize;
        let mut channel = channel.borrow_mut();
        loop {
            match channel.get_metrics_from_sozu() {
                Err(metrics_channel_error) => {
                    error!(
                        "Could not write the metrics request to sozu or receive a response: {:#}",
                        metrics_channel_error
                    );

                    if channel_resurrection_retries < 3 {
                        info!(
                            "Recreating the channel, retry #{}",
                            channel_resurrection_retries
                        );

                        let new_channel = new_sozu_channel().map_err(|new_channel_error| {
                            error!("could not recreate a channel: {:#}", new_channel_error);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                        channel.channel = new_channel;

                        channel_resurrection_retries += 1;
                        continue;
                    }
                    error!("Could not resurrect the channel");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
                Ok(response) => return Ok(format!("{:?}", response)),
            }
        }
    })
}
