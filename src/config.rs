use log::debug;
use std::path::Path;
use thiserror::Error;

use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("can not read config file: {0}")]
    ReadFile(std::io::Error),

    #[error("can not parse config file: {0}")]
    ParseConfig(toml::de::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub ignore_space: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { ignore_space: true }
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
