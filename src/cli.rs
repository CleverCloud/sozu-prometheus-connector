use std::path::PathBuf;

use clap::Parser;

/// A connector to listen on the /metrics route,
/// request metrics from Sōzu that runs on the same machine
/// and return these metrics in a prometheus format
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the configuration file of the prometheus connector,
    /// MUST BE ABSOLUTE
    #[arg(short = 'c', long = "config")]
    pub config: PathBuf,
}
