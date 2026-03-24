use crate::{
    entry::Entry,
    store::Filter,
};
use chrono::{
    DateTime,
    Utc,
};
use rusqlite::{
    Connection,
    OpenFlags,
    OptionalExtension,
    params,
};
use std::{
    path::{
        Path,
        PathBuf,
    },
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::ffi::{
    OsStrExt,
    OsStringExt,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const SCHEMA_VERSION: &str = "1";

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("can not create cache directory for {0}: {1}")]
    CreateCacheDirectory(PathBuf, #[source] std::io::Error),

    #[error("can not open cache database {0}: {1}")]
    OpenDatabase(PathBuf, #[source] rusqlite::Error),

    #[error("can not configure cache database {0}: {1}")]
    ConfigureDatabase(PathBuf, #[source] rusqlite::Error),

    #[error("can not initialize cache schema in {0}: {1}")]
    InitializeSchema(PathBuf, #[source] rusqlite::Error),

    #[error("can not query cache metadata in {0}: {1}")]
    QueryMetadata(PathBuf, #[source] rusqlite::Error),

    #[error("cache schema version mismatch in {0}")]
    SchemaVersionMismatch(PathBuf),

    #[error("can not append entry to cache database {0}: {1}")]
    InsertEntry(PathBuf, #[source] rusqlite::Error),

    #[error("can not rebuild cache database {0}: {1}")]
    SyncDatabase(PathBuf, #[source] rusqlite::Error),

    #[error("can not glob history files from {0}: {1}")]
    InvalidGlob(PathBuf, #[source] glob::PatternError),

    #[error("problem while iterating history glob: {0}")]
    GlobIteration(#[source] glob::GlobError),

    #[error("can not open history csv file {0}: {1}")]
    OpenCsvFile(PathBuf, #[source] std::io::Error),

    #[error("can not read history csv file {0}: {1}")]
    ReadCsvFile(PathBuf, #[source] csv::Error),

    #[error("can not query cache database {0}: {1}")]
    QueryEntries(PathBuf, #[source] rusqlite::Error),

    #[error("can not convert timestamp {0} from cache database")]
    InvalidTimestamp(i64),

    #[error("can not convert session id from cache database")]
    InvalidSessionId(#[from] uuid::Error),
}

pub(crate) fn append_entry(cache_path: &Path, entry: &Entry) -> Result<(), Error> {
    let conn = open_rw(cache_path)?;
    insert_entry(&conn, cache_path, entry)
}

pub(crate) fn get_entries(cache_path: &Path, filter: &Filter<'_>) -> Result<Vec<Entry>, Error> {
    let conn = open_rw(cache_path)?;

    let mut sql = String::from(
        "SELECT hostname, time_finished, time_start, command, pwd, result, session_id, user FROM \
         entries",
    );
    let mut params = Vec::new();

    if let Some(hostname) = filter.get_hostname() {
        sql.push_str(" WHERE hostname = ?");
        params.push(rusqlite::types::Value::from(hostname.clone()));
    }

    sql.push_str(" ORDER BY time_finished DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| Error::QueryEntries(cache_path.to_path_buf(), err))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let session_id: Vec<u8> = row.get(6)?;

            Ok(Entry {
                hostname: row.get(0)?,
                time_finished: time_from_micros(row.get(1)?)?,
                time_start: time_from_micros(row.get(2)?)?,
                command: row.get(3)?,
                pwd: path_from_bytes(row.get(4)?),
                result: row.get(5)?,
                session_id: Uuid::from_slice(&session_id).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Blob,
                        Box::new(err),
                    )
                })?,
                user: row.get(7)?,
            })
        })
        .map_err(|err| Error::QueryEntries(cache_path.to_path_buf(), err))?;

    let mut entries = Vec::new();

    for row in rows {
        let entry = row.map_err(|err| Error::QueryEntries(cache_path.to_path_buf(), err))?;

        if filter.matches_entry(&entry) {
            entries.push(entry);

            if filter.count_limit() > 0 && entries.len() == filter.count_limit() {
                break;
            }
        }
    }

    entries.reverse();

    Ok(entries)
}

pub(crate) fn sync_from_csv(data_dir: &Path, cache_path: &Path) -> Result<(), Error> {
    let mut conn = open_rw(cache_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    tx.execute("DELETE FROM entries", [])
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    let mut insert_stmt = tx
        .prepare(
            "INSERT INTO entries
             (hostname, time_finished, time_start, command, pwd, result, session_id, user)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    let glob_path = data_dir.join("*.csv");
    let glob = glob::glob(&glob_path.to_string_lossy())
        .map_err(|err| Error::InvalidGlob(glob_path.clone(), err))?;

    for path in glob {
        let path = path.map_err(Error::GlobIteration)?;
        let file =
            std::fs::File::open(&path).map_err(|err| Error::OpenCsvFile(path.clone(), err))?;
        let reader = std::io::BufReader::with_capacity(256 * 1024, file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .buffer_capacity(256 * 1024)
            .from_reader(reader);

        for entry in csv_reader.deserialize() {
            let entry: Entry = entry.map_err(|err| Error::ReadCsvFile(path.clone(), err))?;
            insert_entry_with_statement(&mut insert_stmt, cache_path, &entry)?;
        }
    }

    drop(insert_stmt);
    tx.commit()
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn open_rw(cache_path: &Path) -> Result<Connection, Error> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| Error::CreateCacheDirectory(parent.to_path_buf(), err))?;
    }

    let conn = Connection::open_with_flags(
        cache_path,
        OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|err| Error::OpenDatabase(cache_path.to_path_buf(), err))?;

    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;

    initialize_schema(&conn, cache_path)?;

    Ok(conn)
}

fn initialize_schema(conn: &Connection, cache_path: &Path) -> Result<(), Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS entries (
             hostname TEXT NOT NULL,
             time_finished INTEGER NOT NULL,
             time_start INTEGER NOT NULL,
             command TEXT NOT NULL,
             pwd BLOB NOT NULL,
             result INTEGER NOT NULL,
             session_id BLOB NOT NULL,
             user TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS entries_host_finished_idx
             ON entries(hostname, time_finished DESC);
         CREATE INDEX IF NOT EXISTS entries_finished_idx
             ON entries(time_finished DESC);",
    )
    .map_err(|err| Error::InitializeSchema(cache_path.to_path_buf(), err))?;

    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| Error::QueryMetadata(cache_path.to_path_buf(), err))?;

    match version {
        Some(version) if version == SCHEMA_VERSION => Ok(()),
        Some(_) => Err(Error::SchemaVersionMismatch(cache_path.to_path_buf())),
        None => {
            conn.execute(
                "INSERT INTO metadata (key, value) VALUES ('schema_version', ?1)",
                [SCHEMA_VERSION],
            )
            .map_err(|err| Error::InitializeSchema(cache_path.to_path_buf(), err))?;

            Ok(())
        }
    }
}

fn insert_entry(conn: &Connection, cache_path: &Path, entry: &Entry) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO entries
         (hostname, time_finished, time_start, command, pwd, result, session_id, user)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            &entry.hostname,
            entry.time_finished.timestamp_micros(),
            entry.time_start.timestamp_micros(),
            &entry.command,
            path_to_bytes(&entry.pwd),
            i64::from(entry.result),
            entry.session_id.as_bytes().to_vec(),
            &entry.user,
        ],
    )
    .map_err(|err| Error::InsertEntry(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn insert_entry_with_statement(
    stmt: &mut rusqlite::Statement<'_>,
    cache_path: &Path,
    entry: &Entry,
) -> Result<(), Error> {
    stmt.execute(params![
        &entry.hostname,
        entry.time_finished.timestamp_micros(),
        entry.time_start.timestamp_micros(),
        &entry.command,
        path_to_bytes(&entry.pwd),
        i64::from(entry.result),
        entry.session_id.as_bytes().to_vec(),
        &entry.user,
    ])
    .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn time_from_micros(micros: i64) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::from_timestamp_micros(micros).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(Error::InvalidTimestamp(micros)),
        )
    })
}

#[cfg(unix)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}
