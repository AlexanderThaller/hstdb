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

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub command: String,
    pub pwd: PathBuf,
    pub result: usize,
    pub session_id: Uuid,
    pub time_finished: DateTime<Utc>,
    pub time_start: DateTime<Utc>,
    pub user: String,
}

impl Entry {
    pub fn from_messages(start: CommandStart, finish: CommandFinished) -> Self {
        Self {
            command: start.command,
            pwd: start.pwd,
            result: finish.result,
            session_id: start.session_id,
            time_finished: finish.time_stamp,
            time_start: start.time_stamp,
            user: start.user,
        }
    }
}
