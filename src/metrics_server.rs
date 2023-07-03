use axum::http::StatusCode;
use tracing::error;

use crate::{
    prometheus::convert_metrics_to_prometheus,
    sozu_channel::{new_sozu_channel, SozuChannel, SOZU_CHANNEL},
};

pub async fn get_metrics() -> Result<String, StatusCode> {
    let mut channel_opt = SOZU_CHANNEL.lock().await;

    let mut channel_resurrection_retries = 0usize;

    loop {
        if let Some(ref mut channel) = *channel_opt {
            match channel.get_metrics_from_sozu() {
                Err(metrics_channel_error) => {
                    error!(
                        "Could not write the metrics request to Sozu or receive a response: {:#}",
                        metrics_channel_error
                    );

                    if channel_resurrection_retries < 3 {
                        error!(
                            "Recreating the channel, retry #{}",
                            channel_resurrection_retries
                        );

                        let new_channel = new_sozu_channel(&channel.sozu_socket_path).map_err(
                            |new_channel_error| {
                                error!("could not recreate a channel: {:#}", new_channel_error);
                                StatusCode::INTERNAL_SERVER_ERROR
                            },
                        )?;

                        *channel = SozuChannel {
                            channel: new_channel,
                            sozu_socket_path: channel.sozu_socket_path.clone(),
                        };

                        channel_resurrection_retries += 1;
                        continue;
                    }

                    error!("Too many channel resurrection retries");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
                Ok(aggregated_metrics) => {
                    let prometheus_format = convert_metrics_to_prometheus(aggregated_metrics);

                    return Ok(prometheus_format);
                }
            }
        }
    }
}
