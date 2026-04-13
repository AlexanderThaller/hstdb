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
const SCHEMA_VERSION: &str = "4";

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
        "SELECT h.name, e.time_finished, e.time_start, c.name, a.text, p.path, e.result, s.uuid, \
         u.name
         FROM entries e
         JOIN hostnames     h ON h.id = e.hostname_id
         JOIN commands      c ON c.id = e.command_id
         JOIN command_args  a ON a.id = e.args_id
         JOIN pwds          p ON p.id = e.pwd_id
         JOIN sessions      s ON s.id = e.session_id
         JOIN users         u ON u.id = e.user_id",
    );
    let mut params = Vec::new();

    if let Some(hostname) = filter.get_hostname() {
        sql.push_str(" WHERE h.name = ?");
        params.push(rusqlite::types::Value::from(hostname.clone()));
    }

    sql.push_str(" ORDER BY e.time_finished DESC, e.time_start DESC, e.rowid DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| Error::QueryEntries(cache_path.to_path_buf(), err))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let command_name: String = row.get(3)?;
            let args: String = row.get(4)?;
            let session_id: Vec<u8> = row.get(7)?;

            Ok(Entry {
                hostname: row.get(0)?,
                time_finished: time_from_micros(row.get(1)?)?,
                time_start: time_from_micros(row.get(2)?)?,
                command: join_command(&command_name, &args),
                pwd: path_from_bytes(row.get(5)?),
                result: row.get(6)?,
                session_id: Uuid::from_slice(&session_id).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Blob,
                        Box::new(err),
                    )
                })?,
                user: row.get(8)?,
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

    if !filter.is_latest_first() {
        entries.reverse();
    }

    Ok(entries)
}

pub(crate) fn sync_from_csv(data_dir: &Path, cache_path: &Path) -> Result<(), Error> {
    let mut conn = open_rw_for_sync(cache_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    tx.execute_batch(
        "DELETE FROM entries;
         DELETE FROM hostnames;
         DELETE FROM users;
         DELETE FROM commands;
         DELETE FROM command_args;
         DELETE FROM pwds;
         DELETE FROM sessions;",
    )
    .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    let mut stmts = InternStmts::prepare(&tx)
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
            stmts
                .insert_entry(&entry)
                .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;
        }
    }

    drop(stmts);
    tx.commit()
        .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn open_rw(cache_path: &Path) -> Result<Connection, Error> {
    ensure_cache_parent(cache_path)?;

    let conn = open_connection(cache_path)?;
    configure_connection(&conn, cache_path)?;
    initialize_schema(&conn, cache_path)?;

    Ok(conn)
}

fn open_rw_for_sync(cache_path: &Path) -> Result<Connection, Error> {
    ensure_cache_parent(cache_path)?;

    let conn = open_connection(cache_path)?;
    configure_connection(&conn, cache_path)?;

    match initialize_schema(&conn, cache_path) {
        Ok(()) => Ok(conn),
        Err(Error::SchemaVersionMismatch(_)) => {
            reset_schema(&conn, cache_path)?;
            initialize_schema(&conn, cache_path)?;
            Ok(conn)
        }
        Err(err) => Err(err),
    }
}

fn ensure_cache_parent(cache_path: &Path) -> Result<(), Error> {
    if let Some(parent) = cache_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| Error::CreateCacheDirectory(parent.to_path_buf(), err))?;
    }

    Ok(())
}

fn open_connection(cache_path: &Path) -> Result<Connection, Error> {
    Connection::open_with_flags(
        cache_path,
        OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|err| Error::OpenDatabase(cache_path.to_path_buf(), err))
}

fn configure_connection(conn: &Connection, cache_path: &Path) -> Result<(), Error> {
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|err| Error::ConfigureDatabase(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn initialize_schema(conn: &Connection, cache_path: &Path) -> Result<(), Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );",
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

    if let Some(ref v) = version
        && v != SCHEMA_VERSION
    {
        return Err(Error::SchemaVersionMismatch(cache_path.to_path_buf()));
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS hostnames (
             id   INTEGER PRIMARY KEY,
             name TEXT NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS users (
             id   INTEGER PRIMARY KEY,
             name TEXT NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS commands (
             id   INTEGER PRIMARY KEY,
             name TEXT NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS command_args (
             id   INTEGER PRIMARY KEY,
             text TEXT NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS pwds (
             id   INTEGER PRIMARY KEY,
             path BLOB NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS sessions (
             id   INTEGER PRIMARY KEY,
             uuid BLOB NOT NULL UNIQUE
         );
         CREATE TABLE IF NOT EXISTS entries (
             hostname_id   INTEGER NOT NULL REFERENCES hostnames(id),
             time_finished INTEGER NOT NULL,
             time_start    INTEGER NOT NULL,
             command_id    INTEGER NOT NULL REFERENCES commands(id),
             args_id       INTEGER NOT NULL REFERENCES command_args(id),
             pwd_id        INTEGER NOT NULL REFERENCES pwds(id),
             result        INTEGER NOT NULL,
             session_id    INTEGER NOT NULL REFERENCES sessions(id),
             user_id       INTEGER NOT NULL REFERENCES users(id)
         );
         CREATE INDEX IF NOT EXISTS entries_host_time_idx
             ON entries(hostname_id, time_finished DESC, time_start DESC);
         CREATE INDEX IF NOT EXISTS entries_time_idx
             ON entries(time_finished DESC, time_start DESC);",
    )
    .map_err(|err| Error::InitializeSchema(cache_path.to_path_buf(), err))?;

    if version.is_none() {
        conn.execute(
            "INSERT INTO metadata (key, value) VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )
        .map_err(|err| Error::InitializeSchema(cache_path.to_path_buf(), err))?;
    }

    Ok(())
}

fn reset_schema(conn: &Connection, cache_path: &Path) -> Result<(), Error> {
    conn.execute_batch(
        "DROP INDEX IF EXISTS entries_host_finished_idx;
         DROP INDEX IF EXISTS entries_finished_idx;
         DROP INDEX IF EXISTS entries_host_order_idx;
         DROP INDEX IF EXISTS entries_order_idx;
         DROP INDEX IF EXISTS entries_host_time_idx;
         DROP INDEX IF EXISTS entries_time_idx;
         DROP TABLE IF EXISTS entries;
         DROP TABLE IF EXISTS hostnames;
         DROP TABLE IF EXISTS users;
         DROP TABLE IF EXISTS commands;
         DROP TABLE IF EXISTS command_args;
         DROP TABLE IF EXISTS pwds;
         DROP TABLE IF EXISTS sessions;
         DROP TABLE IF EXISTS metadata;",
    )
    .map_err(|err| Error::SyncDatabase(cache_path.to_path_buf(), err))?;

    Ok(())
}

fn insert_entry(conn: &Connection, cache_path: &Path, entry: &Entry) -> Result<(), Error> {
    let map_err = |err| Error::InsertEntry(cache_path.to_path_buf(), err);
    let mut stmts = InternStmts::prepare(conn).map_err(map_err)?;
    stmts.insert_entry(entry).map_err(map_err)
}

struct InternStmts<'conn> {
    insert_hostname: rusqlite::Statement<'conn>,
    select_hostname: rusqlite::Statement<'conn>,
    insert_user: rusqlite::Statement<'conn>,
    select_user: rusqlite::Statement<'conn>,
    insert_command: rusqlite::Statement<'conn>,
    select_command: rusqlite::Statement<'conn>,
    insert_args: rusqlite::Statement<'conn>,
    select_args: rusqlite::Statement<'conn>,
    insert_pwd: rusqlite::Statement<'conn>,
    select_pwd: rusqlite::Statement<'conn>,
    insert_session: rusqlite::Statement<'conn>,
    select_session: rusqlite::Statement<'conn>,
    insert_entry: rusqlite::Statement<'conn>,
}

impl<'conn> InternStmts<'conn> {
    fn prepare(conn: &'conn Connection) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            insert_hostname: conn.prepare("INSERT OR IGNORE INTO hostnames (name) VALUES (?1)")?,
            select_hostname: conn.prepare("SELECT id FROM hostnames WHERE name = ?1")?,
            insert_user: conn.prepare("INSERT OR IGNORE INTO users (name) VALUES (?1)")?,
            select_user: conn.prepare("SELECT id FROM users WHERE name = ?1")?,
            insert_command: conn.prepare("INSERT OR IGNORE INTO commands (name) VALUES (?1)")?,
            select_command: conn.prepare("SELECT id FROM commands WHERE name = ?1")?,
            insert_args: conn.prepare("INSERT OR IGNORE INTO command_args (text) VALUES (?1)")?,
            select_args: conn.prepare("SELECT id FROM command_args WHERE text = ?1")?,
            insert_pwd: conn.prepare("INSERT OR IGNORE INTO pwds (path) VALUES (?1)")?,
            select_pwd: conn.prepare("SELECT id FROM pwds WHERE path = ?1")?,
            insert_session: conn.prepare("INSERT OR IGNORE INTO sessions (uuid) VALUES (?1)")?,
            select_session: conn.prepare("SELECT id FROM sessions WHERE uuid = ?1")?,
            insert_entry: conn.prepare(
                "INSERT INTO entries
                 (hostname_id, time_finished, time_start, command_id, args_id, pwd_id, result, \
                 session_id, user_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?,
        })
    }

    fn insert_entry(&mut self, entry: &Entry) -> Result<(), rusqlite::Error> {
        let hostname_id = intern_with(
            &mut self.insert_hostname,
            &mut self.select_hostname,
            params![&entry.hostname],
        )?;
        let user_id = intern_with(
            &mut self.insert_user,
            &mut self.select_user,
            params![&entry.user],
        )?;
        let (command_name, args) = split_command(&entry.command);
        let command_id = intern_with(
            &mut self.insert_command,
            &mut self.select_command,
            params![command_name],
        )?;
        let args_id = intern_with(&mut self.insert_args, &mut self.select_args, params![args])?;
        let pwd_bytes = path_to_bytes(&entry.pwd);
        let pwd_id = intern_with(
            &mut self.insert_pwd,
            &mut self.select_pwd,
            params![pwd_bytes],
        )?;
        let session_uuid = entry.session_id.as_bytes().as_slice();
        let session_id = intern_with(
            &mut self.insert_session,
            &mut self.select_session,
            params![session_uuid],
        )?;

        self.insert_entry.execute(params![
            hostname_id,
            entry.time_finished.timestamp_micros(),
            entry.time_start.timestamp_micros(),
            command_id,
            args_id,
            pwd_id,
            i64::from(entry.result),
            session_id,
            user_id,
        ])?;

        Ok(())
    }
}

fn intern_with(
    insert: &mut rusqlite::Statement<'_>,
    select: &mut rusqlite::Statement<'_>,
    params: &[&dyn rusqlite::ToSql],
) -> Result<i64, rusqlite::Error> {
    insert.execute(params)?;
    select.query_row(params, |row| row.get(0))
}

fn split_command(command: &str) -> (&str, &str) {
    match command.find(' ') {
        Some(idx) => (&command[..idx], &command[idx..]),
        None => (command, ""),
    }
}

fn join_command(name: &str, args: &str) -> String {
    format!("{name}{args}")
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

#[cfg(test)]
mod tests {
    use super::{
        join_command,
        split_command,
    };

    #[test]
    fn split_and_join_round_trips() {
        for command in [
            "",
            "ls",
            "ls -la",
            "ls ",
            " ",
            "  ls",
            "git  status",
            "git commit -m \"msg\"",
            "echo 'hello world'",
        ] {
            let (name, args) = split_command(command);
            assert_eq!(join_command(name, args), command);
        }
    }

    #[test]
    fn split_command_extracts_first_token() {
        assert_eq!(split_command("git status"), ("git", " status"));
        assert_eq!(split_command("git"), ("git", ""));
        assert_eq!(split_command(""), ("", ""));
        assert_eq!(split_command("git  status"), ("git", "  status"));
        assert_eq!(split_command("ls "), ("ls", " "));
    }
}
