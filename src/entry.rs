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

#[derive(Debug)]
pub struct Entry {
    pub command: String,
    pub pwd: PathBuf,
    pub result: Option<usize>,
    pub session_id: Uuid,
    pub time_end: Option<DateTime<Utc>>,
    pub time_start: DateTime<Utc>,
    pub user: String,
}

#[derive(Error, Debug)]
pub enum Error {
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

    #[error("invalid result: {0}")]
    InvalidResult(std::num::ParseIntError),
}

impl Entry {
    pub fn from_env() -> Result<Self, Error> {
        let command = env::args().into_iter().skip(2).next().unwrap_or_default();

        let pwd = env::current_dir().map_err(Error::GetCurrentDir)?;

        let time_start = Utc::now();
        let time_end = None;

        let user = env::var("USER").map_err(Error::GetUser)?;

        let result = env::var("HISTDB_RS_RETVAL")
            .ok()
            .map(|s| s.parse().map_err(Error::InvalidResult))
            .transpose()?;

        let session_id = match env::var("HISTDB_RS_SESSION_ID") {
            Err(err) => match err {
                env::VarError::NotPresent => Err(Error::MissingSessionID),
                _ => Err(Error::InvalidSessionIDEnvVar(err)),
            },

            Ok(s) => Uuid::parse_str(&s).map_err(Error::InvalidSessionID),
        }?;

        Ok(Self {
            command,
            pwd,
            result,
            session_id,
            time_end,
            time_start,
            user,
        })
    }
}
