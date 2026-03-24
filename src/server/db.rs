use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    path::Path,
    sync::{
        Arc,
        RwLock,
    },
};

use thiserror::Error;
use uuid::Uuid;

use crate::message::CommandStart;

/// Errors returned by the transient server database.
#[derive(Error, Debug)]
pub enum Error {
    /// No in-flight entry exists for the requested session.
    #[error("entry does not exist in db")]
    EntryNotExist,
}

/// Small database used for in-flight commands and disabled
/// sessions.
#[derive(Debug)]
pub struct Db {
    entries: Arc<RwLock<BTreeMap<Uuid, CommandStart>>>,
    disabled_sessions: Arc<RwLock<BTreeSet<Uuid>>>,
}

/// Opens the transient databases used by the server under `path`.
pub fn new(_path: impl AsRef<Path>) -> Result<Db, Error> {
    // TODO: Persistence
    Ok(Db {
        entries: Arc::new(RwLock::new(BTreeMap::new())),
        disabled_sessions: Arc::new(RwLock::new(BTreeSet::new())),
    })
}

impl Db {
    /// Returns whether an in-flight command exists for `uuid`.
    #[must_use]
    pub fn contains_entry(&self, uuid: &Uuid) -> bool {
        self.entries
            .read()
            .expect("Failed to get read lock for entries")
            .contains_key(uuid)
    }

    /// Returns whether history recording is disabled for `uuid`.
    #[must_use]
    pub fn is_session_disabled(&self, uuid: &Uuid) -> bool {
        self.disabled_sessions
            .read()
            .expect("Failed to get read lock for disabled_sessions")
            .contains(uuid)
    }

    /// Stores an in-flight command for the session contained in `entry`.
    pub fn add_entry(&self, entry: &CommandStart) {
        let key = entry.session_id;
        let value = entry.clone();

        self.entries
            .write()
            .expect("Failed to get write lock for entries")
            .insert(key, value);
    }

    /// Removes and returns the in-flight command for `uuid`.
    pub fn remove_entry(&self, uuid: &Uuid) -> Result<CommandStart, Error> {
        let entry = self
            .entries
            .write()
            .expect("Failed to get write lock for entries")
            .remove(uuid)
            .ok_or(Error::EntryNotExist)?;

        Ok(entry)
    }

    /// Marks a session as disabled and removes any in-flight command for it.
    pub fn disable_session(&self, uuid: &Uuid) {
        self.disabled_sessions
            .write()
            .expect("Failed to get write lock for disabled_sessions")
            .insert(*uuid);
    }

    /// Re-enables history recording for `uuid`.
    pub fn enable_session(&self, uuid: &Uuid) {
        self.disabled_sessions
            .write()
            .expect("Failed to get write lock for disabled_sessions")
            .remove(uuid);
    }
}
