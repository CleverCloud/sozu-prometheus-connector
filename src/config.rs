use std::path::PathBuf;

use anyhow::Context;
use config::{Config, File};
use serde::Deserialize;
use sozu_command_lib::config::FileConfig;

#[derive(Deserialize)]
pub struct ConnectorConfig {
    pub sozu_configuration_path: String,
    pub listening_address: String,
}

impl ConnectorConfig {
    pub fn parse_from_file(config_file: &PathBuf) -> anyhow::Result<Self> {
        let config = Config::builder()
            .add_source(config::File::from(config_file.as_path()).required(true))
            .build()
            .with_context(|| format!("Could not build config from path {:?}", config_file))?
            .try_deserialize::<ConnectorConfig>()
            .with_context(|| format!("Could not deserialize file {:?}", config_file))?;

        Ok(config)
    }

    pub fn parse_sozu_config_path(&self) -> PathBuf {
        PathBuf::from(&self.sozu_configuration_path)
    }
}

pub fn get_socket_path_from_sozu_config(config_path: PathBuf) -> anyhow::Result<String> {
    let file_config: FileConfig = Config::builder()
        .add_source(File::from(config_path.as_path()).required(true))
        .build()
        .with_context(|| format!("Could not build config from path {:?}", config_path))?
        .try_deserialize()
        .with_context(|| "Could not deserialize config")?;

    let socket_path_in_the_config = file_config
        .command_socket
        .with_context(|| format!("No command socket path provided in {:?}", config_path))?;

    // if the path is absolute, return as is,
    if socket_path_in_the_config.starts_with('/') {
        return Ok(socket_path_in_the_config);
    }

    // else compute the absolute path
    let config_file_path = PathBuf::from(socket_path_in_the_config);

    let mut parent_path = config_path
        .parent()
        .with_context(|| format!("Could not get parent path of {:?}", config_path))?
        .to_owned();

    parent_path.push(config_file_path);

    // canonicalize to remove dots and double dots
    let total_path = parent_path
        .canonicalize()
        .with_context(|| "Could not canonicalize path")?;
    let socket_path = total_path
        .to_str()
        .with_context(|| "Could not convert the socket path to string")?;
    Ok(socket_path.to_owned())
}
