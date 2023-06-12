use axum::{http::StatusCode, routing::get, Router};
use tracing::error;

use crate::sozu_channel::SOZU_CHANNEL;

pub fn metrics_app() -> Router {
    Router::new().route("/metrics", get(get_metrics))
}

pub async fn get_metrics() -> Result<String, StatusCode> {
    SOZU_CHANNEL.with(|channel| {
        channel
            .borrow_mut()
            .send_metrics_request_to_sozu_and_read_response()
            .map_err(|sozu_error| {
                error!(
                    "Could not write the metrics request to sozu or receive a response: {:#}",
                    sozu_error
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })
    })
}
