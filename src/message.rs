use chrono::{
    DateTime,
    Utc,
};
use std::{
    env,
    path::PathBuf,
};
use thiserror::Error;
use uuid::Uuid;

use crate::config::Config;

/// Messages exchanged between the client-side shell hooks and the server.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Message {
    /// Requests a graceful server shutdown.
    Stop,
    /// Disables history recording for the given session.
    Disable(Uuid),
    /// Re-enables history recording for the given session.
    Enable(Uuid),
    /// Announces that a command has started.
    CommandStart(CommandStart),
    /// Announces that a command has finished.
    CommandFinished(CommandFinished),
}

/// Errors returned while constructing messages from process environment data.
#[derive(Error, Debug)]
pub(crate) enum Error {
    /// Resolving the local hostname failed.
    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    /// Resolving the current working directory failed.
    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),

    /// Reading the current user from the environment failed.
    #[error("can not get current user: {0}")]
    GetUser(env::VarError),

    /// Reading the session-id environment variable failed due to invalid
    /// contents.
    #[error("invalid session id in environment variable: {0}")]
    InvalidSessionIDEnvVar(env::VarError),

    /// Parsing the session id as a UUID failed.
    #[error("invalid session id: {0}")]
    InvalidSessionID(uuid::Error),

    /// No session id was available in the environment.
    #[error("session id is missing")]
    MissingSessionID,

    /// The shell did not export a return value for the finished command.
    #[error("retval is missing")]
    MissingRetval(std::env::VarError),

    /// Parsing the shell return value failed.
    #[error("invalid result: {0}")]
    InvalidResult(std::num::ParseIntError),
}

/// Message payload emitted when a command starts executing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommandStart {
    /// Command line as reported by the shell hook.
    pub(crate) command: String,
    /// Current working directory at command start.
    pub(crate) pwd: PathBuf,
    /// Session identifier used to pair start and finish notifications.
    pub(crate) session_id: Uuid,
    /// Time at which the command started.
    pub(crate) time_stamp: DateTime<Utc>,
    /// User that started the command.
    pub(crate) user: String,
    /// Hostname recorded for the command.
    pub(crate) hostname: String,
}

impl CommandStart {
    /// Builds a start message from the current process environment and
    /// configuration.
    pub(crate) fn from_env(config: &Config, command: String) -> Result<Self, Error> {
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

/// Message payload emitted when a command finishes executing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CommandFinished {
    /// Session identifier of the command that finished.
    pub(crate) session_id: Uuid,
    /// Time at which the command finished.
    pub(crate) time_stamp: DateTime<Utc>,
    /// Shell exit status of the finished command.
    pub(crate) result: u16,
}

impl CommandFinished {
    /// Builds a finish message from the current process environment.
    pub(crate) fn from_env() -> Result<Self, Error> {
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

/// Reads and parses `HISTDB_RS_SESSION_ID` from the current environment.
pub(crate) fn session_id_from_env() -> Result<Uuid, Error> {
    match env::var("HISTDB_RS_SESSION_ID") {
        Err(err) => match err {
            env::VarError::NotPresent => Err(Error::MissingSessionID),
            env::VarError::NotUnicode(_) => Err(Error::InvalidSessionIDEnvVar(err)),
        },

        Ok(s) => Uuid::parse_str(&s).map_err(Error::InvalidSessionID),
    }
}
