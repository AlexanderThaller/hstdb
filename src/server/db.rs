use crate::message::CommandStart;
use bincode::serde::{
    BorrowCompat,
    Compat,
};
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
    SerializeData(bincode::error::EncodeError),

    #[error("can not deserialize entry: {0}")]
    DeserializeEntry(bincode::error::DecodeError),

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
        let key = Self::serialize(BorrowCompat(uuid))?;
        let contains = self.entries.contains_key(key)?;

        Ok(contains)
    }

    pub fn is_session_disabled(&self, uuid: &Uuid) -> Result<bool, Error> {
        let key = Self::serialize(BorrowCompat(uuid))?;
        let contains = self.disabled_sessions.contains_key(key)?;

        Ok(contains)
    }

    pub fn add_entry(&self, entry: &CommandStart) -> Result<(), Error> {
        let key = Self::serialize(BorrowCompat(&entry.session_id))?;
        let value = Self::serialize(BorrowCompat(entry))?;

        self.entries.insert(key, value)?;

        Ok(())
    }

    pub fn remove_entry(&self, uuid: &Uuid) -> Result<CommandStart, Error> {
        let key = Self::serialize(BorrowCompat(uuid))?;

        let data = self.entries.remove(key)?.ok_or(Error::EntryNotExist)?;

        let entry = Self::deserialize_entry(&data)?;

        Ok(entry)
    }

    pub fn disable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(BorrowCompat(uuid))?;
        let value = Self::serialize(true)?;

        self.disabled_sessions.insert(key, value)?;

        self.remove_entry(uuid)?;

        Ok(())
    }

    pub fn enable_session(&self, uuid: &Uuid) -> Result<(), Error> {
        let key = Self::serialize(BorrowCompat(uuid))?;

        self.disabled_sessions.remove(&key)?;

        Ok(())
    }

    fn serialize(data: impl bincode::Encode) -> Result<Vec<u8>, Error> {
        let bytes = bincode::encode_to_vec(&data, bincode::config::standard())
            .map_err(Error::SerializeData)?;

        Ok(bytes)
    }

    fn deserialize_entry(data: &sled::IVec) -> Result<CommandStart, Error> {
        let (entry, _): (Compat<CommandStart>, _) =
            bincode::decode_from_slice(data, bincode::config::standard())
                .map_err(Error::DeserializeEntry)?;

        Ok(entry.0)
    }
}
