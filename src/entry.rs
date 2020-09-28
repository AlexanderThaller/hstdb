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
    pub result: usize,
    pub session_id: Uuid,
    pub user: String,
}

impl Entry {
    #[allow(clippy::missing_const_for_fn)]
    pub fn from_messages(start: CommandStart, finish: &CommandFinished) -> Self {
        Self {
            command: start.command,
            pwd: start.pwd,
            result: finish.result,
            session_id: start.session_id,
            time_finished: finish.time_stamp,
            time_start: start.time_stamp,
            user: start.user,
            hostname: start.hostname,
        }
    }
}
