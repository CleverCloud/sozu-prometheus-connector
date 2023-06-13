use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use config::{Config, File};
use sozu_command_lib::config::FileConfig;

pub fn parse_connector_config_file_for_sozu_config_path(
    config_file: &PathBuf,
) -> anyhow::Result<PathBuf> {
    let settings = Config::builder()
        .add_source(config::File::from(config_file.as_path()).required(true))
        .build()
        .with_context(|| format!("Could not build config from path {:?}", config_file))?
        .try_deserialize::<HashMap<String, String>>()
        .with_context(|| format!("Could not deserialize file {:?}", config_file))?;

    let sozu_configuration_path = settings
        .get("sozu_configuration_path")
        .with_context(|| "No parameter 'sozu_configuration_path' in the config")?;
    let path_buf = PathBuf::from(sozu_configuration_path);
    Ok(path_buf)
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

    // else concatenate it with the config_path
    let relative_path = PathBuf::from(socket_path_in_the_config);

    let mut absolute_path = config_path
        .parent()
        .with_context(|| format!("Could not get parent path of {:?}", config_path))?
        .to_owned();
    absolute_path.push(relative_path);

    // canonicalize to remove dots and double dots
    let total_path = absolute_path
        .canonicalize()
        .with_context(|| "Could not canonicalize path")?;
    let socket_path = total_path
        .to_str()
        .with_context(|| "Could not convert the socket path to string")?;
    Ok(socket_path.to_owned())
}
