use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    io::{
        Read,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        RwLock,
    },
};

use color_eyre::eyre::WrapErr;
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
#[derive(Debug, Clone)]
pub struct Db {
    entries: Arc<RwLock<BTreeMap<Uuid, CommandStart>>>,
    disabled_sessions: Arc<RwLock<BTreeSet<Uuid>>>,

    entries_path: PathBuf,
    disabled_sessions_path: PathBuf,
}

/// Opens the transient databases used by the server under `path`.
pub fn new(path: impl AsRef<Path>) -> color_eyre::Result<Db> {
    let entries_path = path.as_ref().join("entries.bitcode");
    let disabled_sessions_path = path.as_ref().join("disabled_sessions.bitcode");

    let entries = if entries_path.exists() {
        let file = std::fs::File::open(&entries_path).wrap_err("Failed to open entries file")?;
        let mut reader = std::io::BufReader::new(file);
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .wrap_err("Failed to read entries file")?;

        bitcode::deserialize(&data).wrap_err("Failed to deserialize entries file")?
    } else {
        BTreeMap::new()
    };

    let disabled_sessions = if disabled_sessions_path.exists() {
        let file = std::fs::File::open(&disabled_sessions_path)
            .wrap_err("Failed to open disabled sessions file")?;
        let mut reader = std::io::BufReader::new(file);
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .wrap_err("Failed to read disabled sessions file")?;

        bitcode::deserialize(&data).wrap_err("Failed to deserialize disabled sessions file")?
    } else {
        BTreeSet::new()
    };

    let entries = Arc::new(RwLock::new(entries));
    let disabled_sessions = Arc::new(RwLock::new(disabled_sessions));

    Ok(Db {
        entries,
        disabled_sessions,

        entries_path,
        disabled_sessions_path,
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
    pub fn remove_entry(&self, uuid: &Uuid) -> color_eyre::Result<CommandStart> {
        let entry = self
            .entries
            .write()
            .expect("Failed to get write lock for entries")
            .remove(uuid)
            .ok_or(Error::EntryNotExist)?;

        self.persist_entries()
            .wrap_err("Failed to persist entries after removing entry")?;

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

    /// Persists the database to disk.
    pub fn persist(&self) -> color_eyre::Result<()> {
        self.persist_entries()
            .wrap_err("Failed to persist entries")?;

        self.persist_disabled_sessions()
            .wrap_err("Failed to persist disabled sessions")?;

        Ok(())
    }

    fn persist_to_file<P, S>(path: P, data: &S) -> color_eyre::Result<()>
    where
        P: AsRef<Path>,
        S: serde::ser::Serialize,
    {
        let parent = path
            .as_ref()
            .parent()
            .ok_or_else(|| color_eyre::eyre::eyre!("No parent directory for path"))?;

        std::fs::create_dir_all(parent).wrap_err("Failed to create parent directory for file")?;

        let file = std::fs::File::create(path).wrap_err("Failed to create file")?;

        let data = bitcode::serialize(data).wrap_err("Failed to serialize data")?;

        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(&data).wrap_err("Failed to write file")?;
        writer.flush().wrap_err("Failed to flush file")?;

        Ok(())
    }

    fn persist_entries(&self) -> color_eyre::Result<()> {
        let entries = self
            .entries
            .read()
            .expect("Failed to get read lock for entries");

        Self::persist_to_file(&self.entries_path, &*entries)
            .wrap_err("Failed to persist entries")?;

        Ok(())
    }

    fn persist_disabled_sessions(&self) -> color_eyre::Result<()> {
        let disabled_sessions = self
            .disabled_sessions
            .read()
            .expect("Failed to get read lock for disabled_sessions");

        Self::persist_to_file(&self.disabled_sessions_path, &*disabled_sessions)
            .wrap_err("Failed to persist disabled sessions")?;

        Ok(())
    }
}
