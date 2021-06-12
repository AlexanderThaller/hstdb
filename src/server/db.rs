use crate::message::CommandStart;
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not open entries database: {0}")]
    OpenEntriesDatabase(sled::Error),

    #[error("can not open disabled_sessions database: {0}")]
    OpenDisabledSessionsDatabase(sled::Error),

    #[error("can not serialize data: {0}")]
    SerializeData(bincode::Error),

    #[error("can not deserialize entry: {0}")]
    DeserializeEntry(bincode::Error),

    #[error("{0}")]
    Sled(#[from] sled::Error),

    #[error("entry does not exist in db")]
    EntryNotExist,
}

pub fn new(path: impl AsRef<Path>) -> Result<Db, Error> {
    let entries = sled::open(path.as_ref().join("entries")).map_err(Error::OpenEntriesDatabase)?;
    let disabled_sessions = sled::open(path.as_ref().join("disabled_sessions"))
        .map_err(Error::OpenDisabledSessionsDatabase)?;

    Ok(Db {
        entries,
        disabled_sessions,
    })
}

pub struct Db {
    entries: sled::Db,
    disabled_sessions: sled::Db,
}

impl Db {
    pub fn contains_entry(&self, uuid: &Uuid) -> Result<bool, Error> {
        let key = Self::serialize(uuid)?;
        let contains = self.entries.contains_key(key)?;

        Ok(contains)
    }

    pub fn is_session_disabled(&self, uuid: &Uuid) -> Result<bool, Error> {
        let key = Self::serialize(uuid)?;
        let contains = self.disabled_sessions.contains_key(key)?;

        Ok(contains)
    }

    pub fn add_entry(&self, entry: &CommandStart) -> Result<(), Error> {
        let key = Self::serialize(&entry.session_id)?;
        let value = Self::serialize(&entry)?;

        self.entries.insert(key, value)?;

        Ok(())
    }

    pub fn remove_entry(&self, uuid: &Uuid) -> Result<CommandStart, Error> {
        let key = Self::serialize(uuid)?;

        let data = self.entries.remove(key)?.ok_or(Error::EntryNotExist)?;

        let entry = Self::deserialize_entry(&data)?;

        Ok(entry)
    }

    pub fn disable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(uuid)?;
        let value = Self::serialize(true)?;

        self.disabled_sessions.insert(key, value)?;

        self.remove_entry(uuid)?;

        Ok(())
    }

    pub fn enable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(uuid)?;

        self.disabled_sessions.remove(&key)?;

        Ok(())
    }

    fn serialize(data: impl Serialize) -> Result<Vec<u8>, Error> {
        let bytes = bincode::serialize(&data).map_err(Error::SerializeData)?;

        Ok(bytes)
    }

    fn deserialize_entry(data: &sled::IVec) -> Result<CommandStart, Error> {
        let entry = bincode::deserialize(data).map_err(Error::DeserializeEntry)?;

        Ok(entry)
    }
}
