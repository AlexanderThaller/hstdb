use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    env,
    path::PathBuf,
};
use thiserror::Error;
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Stop,

    Disable(Uuid),

    Enable(Uuid),

    CommandStart(CommandStart),

    CommandFinished(CommandFinished),
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),

    #[error("can not get current user: {0}")]
    GetUser(env::VarError),

    #[error("invalid session id in environment variable: {0}")]
    InvalidSessionIDEnvVar(env::VarError),

    #[error("invalid session id: {0}")]
    InvalidSessionID(uuid::Error),

    #[error("session id is missing")]
    MissingSessionID,

    #[error("retval is missing")]
    MissingRetval(std::env::VarError),

    #[error("invalid result: {0}")]
    InvalidResult(std::num::ParseIntError),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandStart {
    pub command: String,
    pub pwd: PathBuf,
    pub session_id: Uuid,
    pub time_stamp: DateTime<Utc>,
    pub user: String,
    pub hostname: String,
}

impl CommandStart {
    pub fn from_env(config: &Config, command: String) -> Result<Self, Error> {
        let pwd = env::current_dir().map_err(Error::GetCurrentDir)?;

        let time_stamp = Utc::now();

        let user = env::var("USER").map_err(Error::GetUser)?;

        let session_id = session_id_from_env()?;

        let hostname = if let Some(hostname) = config.hostname.clone() {
            hostname
        } else {
            hostname::get()
                .map_err(Error::GetHostname)?
                .to_string_lossy()
                .to_string()
        };

        Ok(Self {
            command,
            pwd,
            session_id,
            time_stamp,
            user,
            hostname,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandFinished {
    pub session_id: Uuid,
    pub time_stamp: DateTime<Utc>,
    pub result: u16,
}

impl CommandFinished {
    pub fn from_env() -> Result<Self, Error> {
        let time_stamp = Utc::now();

        let session_id = session_id_from_env()?;

        let result = env::var("HISTDB_RS_RETVAL")
            .map_err(Error::MissingRetval)?
            .parse()
            .map_err(Error::InvalidResult)?;

        Ok(Self {
            session_id,
            time_stamp,
            result,
        })
    }
}

pub fn session_id_from_env() -> Result<Uuid, Error> {
    match env::var("HISTDB_RS_SESSION_ID") {
        Err(err) => match err {
            env::VarError::NotPresent => Err(Error::MissingSessionID),
            env::VarError::NotUnicode(_) => Err(Error::InvalidSessionIDEnvVar(err)),
        },

        Ok(s) => Uuid::parse_str(&s).map_err(Error::InvalidSessionID),
    }
}
