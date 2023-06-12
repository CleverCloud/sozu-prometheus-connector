use std::cell::RefCell;

use anyhow::{self, bail, Context};
use sozu_command_lib::{
    channel::Channel,
    proto::command::{
        request::RequestType, QueryMetricsOptions, Request, Response, ResponseStatus,
    },
};
use tracing::info;

// TODO: replace this path with some env variable or config or something
const SOZU_SOCKET_PATH: &str = "/home/emmanuel/clever/sozu_for_the_win/github_repo/bin/sozu.sock";

thread_local! {
    pub static SOZU_CHANNEL: RefCell<SozuChannel> = RefCell::new(SozuChannel::new(SOZU_SOCKET_PATH));
}

pub struct SozuChannel {
    channel: Channel<Request, Response>,
}

// todo: replace with sozu_command_lib::config default values when bumping the dependency
const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1_000_000;
const DEFAULT_MAX_COMMAND_BUFFER_SIZE: usize = 2_000_000;

impl SozuChannel {
    pub fn new(sozu_socket_path: &str) -> Self {
        let mut channel: Channel<Request, Response> = Channel::from_path(
            sozu_socket_path,
            DEFAULT_COMMAND_BUFFER_SIZE,
            DEFAULT_MAX_COMMAND_BUFFER_SIZE,
        )
        .expect(&format!(
            "Could not create a sozu channel from path {}",
            sozu_socket_path
        ));

        channel
            .blocking()
            .expect("Could not block the sozu channel");

        Self { channel }
    }

    pub fn send_metrics_request_to_sozu_and_read_response(&mut self) -> anyhow::Result<String> {
        let metrics_request = Request {
            request_type: Some(RequestType::QueryMetrics(QueryMetricsOptions::default())),
        };

        self.channel
            .write_message(&metrics_request)
            .with_context(|| "Could not write metrics request on the sozu channel")?;

        loop {
            let response = self
                .channel
                .read_message()
                .with_context(|| "failed to read message on the sozu channel ")?;
            match response.status() {
                ResponseStatus::Processing => info!("Sozu is processing…"),
                ResponseStatus::Failure => bail!(response.message),
                ResponseStatus::Ok => {
                    info!("Sozu replied with {:?}", response.content);
                    return Ok(format!("{:?}", response.content));
                }
            }
        }
    }
}
