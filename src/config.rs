use log::{
    debug,
    LevelFilter,
};
use std::path::Path;
use thiserror::Error;

use serde::Deserialize;

#[derive(Debug, Error)]
pub enum Error {
    #[error("can not read config file: {0}")]
    ReadFile(std::io::Error),

    #[error("can not parse config file: {0}")]
    ParseConfig(toml::de::Error),
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Then true disables recording commands that start with a space.
    pub ignore_space: bool,

    /// The log level to run under.
    pub log_level: LevelFilter,

    /// The hostname that should be used when writing an entry. If
    /// unset will dynamically get the hostname from the system.
    pub hostname: Option<String>,
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
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        if !path.as_ref().is_file() {
            debug!("no config file found using default");
            return Ok(Self::default());
        }

        let config_data = std::fs::read_to_string(path).map_err(Error::ReadFile)?;
        let config = toml::de::from_str(&config_data).map_err(Error::ParseConfig)?;

        Ok(config)
    }
}
