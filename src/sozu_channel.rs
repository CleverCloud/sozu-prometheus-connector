use std::cell::RefCell;

use anyhow::{self, bail, Context};
use sozu_command_lib::{
    channel::Channel,
    proto::command::{
        request::RequestType, response_content::ContentType, AggregatedMetrics,
        QueryMetricsOptions, Request, Response, ResponseContent, ResponseStatus,
    },
};
use tracing::{debug, error, info};

// TODO: replace this path with some env variable or config or something
const SOZU_SOCKET_PATH: &str = "/home/emmanuel/clever/sozu_for_the_win/github_repo/bin/sozu.sock";

thread_local! {
    pub static SOZU_CHANNEL: RefCell<SozuChannel> = RefCell::new(SozuChannel::new());
}

pub struct SozuChannel {
    pub channel: Channel<Request, Response>,
}

// todo: replace with sozu_command_lib::config default values when bumping the dependency
const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1_000_000;
const DEFAULT_MAX_COMMAND_BUFFER_SIZE: usize = 2_000_000;

impl SozuChannel {
    pub fn new() -> Self {
        info!("Creating the sozu channel, this was called by the local thread");
        Self {
            channel: new_sozu_channel().expect("Could not create channel"),
        }
    }

    pub fn get_metrics_from_sozu(&mut self) -> anyhow::Result<AggregatedMetrics> {
        let metrics_request = Request {
            request_type: Some(RequestType::QueryMetrics(QueryMetricsOptions::default())),
        };

        debug!("handling the writeable event on the channel");
        self.channel
            .handle_events(sozu_command_lib::ready::Ready::writable());

        debug!("writing metrics request on the Sozu channel");
        self.channel
            .write_message(&metrics_request)
            .with_context(|| "Could not write metrics request on the sozu channel")?;

        debug!("handling the readable event on the channel");
        self.channel
            .handle_events(sozu_command_lib::ready::Ready::readable());

        loop {
            debug!("Awaiting a response from sozu");
            let response = self
                .channel
                .read_message_blocking_timeout(Some(std::time::Duration::from_millis(5000)))
                .with_context(|| "failed to read message on the sozu channel ")?;
            match response.status() {
                ResponseStatus::Processing => info!("Sozu is processing…"),
                ResponseStatus::Failure => bail!(response.message),
                ResponseStatus::Ok => {
                    info!("Sozu replied with {:?}", response.content);
                    if let Some(ResponseContent {
                        content_type: Some(ContentType::Metrics(aggregated_metrics)),
                    }) = response.content
                    {
                        return Ok(aggregated_metrics);
                    } else {
                        bail!("Wrong or empty response from sozu");
                    }
                }
            }
        }
    }
}

pub fn new_sozu_channel() -> anyhow::Result<Channel<Request, Response>> {
    info!("Creating new sozu channel");

    let mut channel: Channel<Request, Response> = Channel::from_path(
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

    channel
        .blocking()
        .with_context(|| "Could not block the sozu channel")?;
    Ok(channel)
}
