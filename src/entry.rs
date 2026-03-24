use crate::message::{
    CommandFinished,
    CommandStart,
};
use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::path::PathBuf;
use uuid::Uuid;

/// A fully materialized shell history entry persisted by `hstdb`.
#[derive(Debug, Serialize, Deserialize, Ord, PartialOrd, PartialEq, Eq)]
pub struct Entry {
    /// Timestamp at which the command finished.
    pub time_finished: DateTime<Utc>,
    /// Timestamp at which the command started.
    pub time_start: DateTime<Utc>,
    /// Hostname on which the command ran.
    pub hostname: String,
    /// Command text after normalization.
    pub command: String,
    /// Working directory in which the command ran.
    pub pwd: PathBuf,
    /// Exit status reported by the shell.
    pub result: u16,
    /// Session identifier used to correlate start and finish messages.
    pub session_id: Uuid,
    /// User that ran the command.
    pub user: String,
}

impl Entry {
    /// Builds a persistent entry from the corresponding start and finish
    /// messages.
    #[must_use]
    pub fn from_messages(start: CommandStart, finish: &CommandFinished) -> Self {
        let command = start.command.trim_end();

        let command = command
            .strip_suffix("\\r\\n")
            .or_else(|| command.strip_suffix("\\n"))
            .unwrap_or(command)
            .to_string();

        let user = start.user.trim().to_string();
        let hostname = start.hostname.trim().to_string();

        Self {
            time_finished: finish.time_stamp,
            time_start: start.time_stamp,
            hostname,
            command,
            pwd: start.pwd,
            result: finish.result,
            session_id: start.session_id,
            user,
        }
    }
}
