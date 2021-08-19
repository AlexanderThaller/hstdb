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
    /// Then true disables recording commands that start with a space
    pub ignore_space: bool,

    /// The log level to run under
    pub log_level: LevelFilter,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore_space: true,
            log_level: LevelFilter::Warn,
        }
    }
}

impl Config {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        if !path.as_ref().is_file() {
            debug!("no config file found using default");
            return Ok(Self::default());
        }

        let config_data = std::fs::read(path).map_err(Error::ReadFile)?;
        let config = toml::de::from_slice(&config_data).map_err(Error::ParseConfig)?;

        Ok(config)
    }
}
