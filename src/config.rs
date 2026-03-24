use log::{
    LevelFilter,
    debug,
};
use std::path::Path;
use thiserror::Error;

use serde::Deserialize;

/// Errors returned while loading a configuration file.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// Reading the configuration file from disk failed.
    #[error("can not read config file: {0}")]
    ReadFile(std::io::Error),

    /// Parsing the TOML configuration file failed.
    #[error("can not parse config file: {0}")]
    ParseConfig(toml::de::Error),
}

/// User-configurable runtime settings for `hstdb`.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    /// Then true disables recording commands that start with a space.
    pub(crate) ignore_space: bool,

    /// The log level to run under.
    pub(crate) log_level: LevelFilter,

    /// The hostname that should be used when writing an entry. If
    /// unset will dynamically get the hostname from the system.
    pub(crate) hostname: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore_space: true,
            log_level: LevelFilter::Warn,
            hostname: None,
        }
    }
}

impl Config {
    /// Loads a config from `path`, returning defaults when the file does not
    /// exist.
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        if !path.as_ref().is_file() {
            debug!("no config file found using default");
            return Ok(Self::default());
        }

        let config_data = std::fs::read_to_string(path).map_err(Error::ReadFile)?;
        let config = toml::de::from_str(&config_data).map_err(Error::ParseConfig)?;

        Ok(config)
    }
}
