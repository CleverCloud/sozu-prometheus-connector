use anyhow::{self, bail, Context};
use sozu_command_lib::{
    channel::Channel,
    proto::command::{
        request::RequestType, response_content::ContentType, AggregatedMetrics,
        QueryMetricsOptions, Request, Response, ResponseContent, ResponseStatus,
    },
};
use tokio::sync::Mutex;
use tracing::{debug, info};

// todo: replace with sozu_command_lib::config default values when bumping the dependency
const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1_000_000;
const DEFAULT_MAX_COMMAND_BUFFER_SIZE: usize = 2_000_000;

lazy_static::lazy_static! {
    /// a mutex containing a sozu channel
    pub static ref SOZU_CHANNEL: Mutex<Option<SozuChannel>> = Mutex::new(None);
}

/// a sozu channel to be placed in a mutex and recreated when needed
pub struct SozuChannel {
    pub channel: Channel<Request, Response>,
    /// usefull to recreate the channel
    pub sozu_socket_path: String,
}

/// puts a sozu channel in the mutex, on startup
pub async fn initialize_sozu_channel(sozu_socket_path: &str) -> anyhow::Result<()> {
    let mut channel = SOZU_CHANNEL.lock().await;
    if channel.is_none() {
        *channel = Some(SozuChannel::new(sozu_socket_path)?);
    }
    Ok(())
}

impl SozuChannel {
    pub fn new(sozu_socket_path: &str) -> anyhow::Result<Self> {
        info!("Creating the sozu channel, this was called by the local thread");
        let channel =
            new_sozu_channel(sozu_socket_path).with_context(|| "Could not create channel")?;
        Ok(Self {
            channel,
            sozu_socket_path: sozu_socket_path.to_owned(),
        })
    }

    pub fn get_metrics_from_sozu(&mut self) -> anyhow::Result<AggregatedMetrics> {
        let metrics_request = Request {
            request_type: Some(RequestType::QueryMetrics(QueryMetricsOptions::default())),
        };

        debug!("writing metrics request on the Sozu channel");
        self.channel
            .write_message(&metrics_request)
            .with_context(|| "Could not write metrics request on the sozu channel")?;

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

pub fn new_sozu_channel(sozu_socket_path: &str) -> anyhow::Result<Channel<Request, Response>> {
    info!("Creating new sozu channel");

    let mut channel: Channel<Request, Response> = Channel::from_path(
        sozu_socket_path,
        DEFAULT_COMMAND_BUFFER_SIZE,
        DEFAULT_MAX_COMMAND_BUFFER_SIZE,
    )
    .with_context(|| {
        format!(
            "Could not create a sozu channel from path {}",
            sozu_socket_path
        )
    })?;

    channel
        .blocking()
        .with_context(|| "Could not block the sozu channel")?;
    Ok(channel)
}
