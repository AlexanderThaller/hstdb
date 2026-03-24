use crate::message::CommandStart;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

/// Errors returned by the transient server database.
#[derive(Error, Debug)]
pub enum Error {
    /// Opening the in-flight command database failed.
    #[error("can not open entries database: {0}")]
    OpenEntriesDatabase(sled::Error),

    /// Opening the disabled-session database failed.
    #[error("can not open disabled_sessions database: {0}")]
    OpenDisabledSessionsDatabase(sled::Error),

    /// Serializing a key or value before storage failed.
    #[error("can not serialize data: {0}")]
    SerializeData(bitcode::Error),

    /// Deserializing a stored command entry failed.
    #[error("can not deserialize entry: {0}")]
    DeserializeEntry(bitcode::Error),

    /// An underlying `sled` operation failed.
    #[error("{0}")]
    Sled(#[from] sled::Error),

    /// No in-flight entry exists for the requested session.
    #[error("entry does not exist in db")]
    EntryNotExist,
}

/// Opens the transient databases used by the server under `path`.
pub fn new(path: impl AsRef<Path>) -> Result<Db, Error> {
    let entries = sled::open(path.as_ref().join("entries")).map_err(Error::OpenEntriesDatabase)?;
    let disabled_sessions = sled::open(path.as_ref().join("disabled_sessions"))
        .map_err(Error::OpenDisabledSessionsDatabase)?;

    Ok(Db {
        entries,
        disabled_sessions,
    })
}

/// Small `sled`-backed database used for in-flight commands and disabled
/// sessions.
#[derive(Debug)]
pub struct Db {
    entries: sled::Db,
    disabled_sessions: sled::Db,
}

impl Db {
    /// Returns whether an in-flight command exists for `uuid`.
    pub fn contains_entry(&self, uuid: &Uuid) -> Result<bool, Error> {
        let key = Self::serialize(uuid)?;
        let contains = self.entries.contains_key(key)?;

        Ok(contains)
    }

    /// Returns whether history recording is disabled for `uuid`.
    pub fn is_session_disabled(&self, uuid: &Uuid) -> Result<bool, Error> {
        let key = Self::serialize(uuid)?;
        let contains = self.disabled_sessions.contains_key(key)?;

        Ok(contains)
    }

    /// Stores an in-flight command for the session contained in `entry`.
    pub fn add_entry(&self, entry: &CommandStart) -> Result<(), Error> {
        let key = Self::serialize(entry.session_id)?;
        let value = Self::serialize(entry)?;

        self.entries.insert(key, value)?;

        Ok(())
    }

    /// Removes and returns the in-flight command for `uuid`.
    pub fn remove_entry(&self, uuid: &Uuid) -> Result<CommandStart, Error> {
        let key = Self::serialize(uuid)?;

        let data = self.entries.remove(key)?.ok_or(Error::EntryNotExist)?;

        let entry = Self::deserialize_entry(&data)?;

        Ok(entry)
    }

    /// Marks a session as disabled and removes any in-flight command for it.
    pub fn disable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(uuid)?;
        let value = Self::serialize(true)?;

        self.disabled_sessions.insert(key, value)?;

        self.remove_entry(uuid)?;

        Ok(())
    }

    /// Re-enables history recording for `uuid`.
    pub fn enable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(uuid)?;

        self.disabled_sessions.remove(&key)?;

        Ok(())
    }

    fn serialize(data: impl serde::Serialize) -> Result<Vec<u8>, Error> {
        let bytes = bitcode::serialize(&data).map_err(Error::SerializeData)?;

        Ok(bytes)
    }

    fn deserialize_entry(data: &sled::IVec) -> Result<CommandStart, Error> {
        bitcode::deserialize(data).map_err(Error::DeserializeEntry)
    }
}
