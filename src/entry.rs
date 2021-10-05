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

#[derive(Debug, Serialize, Deserialize, Ord, PartialOrd, PartialEq, Eq)]
pub struct Entry {
    pub time_finished: DateTime<Utc>,
    pub time_start: DateTime<Utc>,
    pub hostname: String,
    pub command: String,
    pub pwd: PathBuf,
    pub result: u16,
    pub session_id: Uuid,
    pub user: String,
}

impl Entry {
    pub fn from_messages(start: CommandStart, finish: &CommandFinished) -> Self {
        dbg!(&start.command);

        let command = start.command.trim_end();

        let command = command
            .strip_suffix("\\r\\n")
            .or_else(|| command.strip_suffix("\\n"))
            .unwrap_or(command)
            .to_string();

        let user = start.user.trim().to_string();
        let hostname = start.hostname.trim().to_string();

        dbg!(&command);

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
