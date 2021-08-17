use crate::{
    client,
    message,
    server,
    store,
};
use chrono::{
    DateTime,
    Utc,
};
#[cfg(feature = "histdb-import")]
use log::info;
use log::warn;
#[cfg(feature = "histdb-import")]
use rusqlite::params;
#[cfg(feature = "histdb-import")]
use std::convert::TryInto;
use std::{
    io::BufRead,
    path::{
        Path,
        PathBuf,
    },
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Client(#[from] client::Error),

    #[error("{0}")]
    Message(#[from] message::Error),

    #[error("{0}")]
    Server(#[from] server::Error),

    #[error("{0}")]
    Store(#[from] store::Error),

    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[cfg(feature = "histdb-import")]
    #[error("can not open sqlite database: {0}")]
    OpenSqliteDatabase(rusqlite::Error),

    #[cfg(feature = "histdb-import")]
    #[error("can not prepare sqlite query to get entries: {0}")]
    PrepareSqliteQuery(rusqlite::Error),

    #[cfg(feature = "histdb-import")]
    #[error("can not convert sqlite row: {0}")]
    ConvertSqliteRow(rusqlite::Error),

    #[cfg(feature = "histdb-import")]
    #[error("can not collect entries from sqlite query: {0}")]
    CollectEntries(rusqlite::Error),

    #[cfg(feature = "histdb-import")]
    #[error("can not convert exit status from sqlite: {0}")]
    ConvertExitStatus(std::num::TryFromIntError),

    #[error("can not open histfile: {0}")]
    OpenHistfile(std::io::Error),

    #[error("accumulator fortime finished is none")]
    TimeFinishedAccumulatorNone,

    #[error("accumulator for result is none")]
    ResultAccumulatorNone,

    #[error("accumulator for command is none")]
    CommandAccumulatorNone,

    #[error("did not find timestamp in histfile line {0}")]
    NoTimestamp(usize),

    #[error("did not find result code in histfile line {0}")]
    NoCode(usize),

    #[error("can not parse timestamp as number from histfile line {1}: {0}")]
    ParseTimestamp(std::num::ParseIntError, usize),

    #[error("can not parse returncode from histfile line {1}: {0}")]
    ParseResultCode(std::num::ParseIntError, usize),

    #[error("can not get base directories")]
    BaseDirectory,

    #[error("can not get current user: {0}")]
    GetUser(std::env::VarError),
}

#[cfg(feature = "histdb-import")]
pub fn histdb(import_file: impl AsRef<Path>, data_dir: PathBuf) -> Result<(), Error> {
    #[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
    struct DBEntry {
        session: i64,
        start_time: i64,
        duration: Option<i64>,
        exit_status: Option<i64>,
        hostname: String,
        pwd: String,
        command: String,
    }

    let db = rusqlite::Connection::open(&import_file).map_err(Error::OpenSqliteDatabase)?;

    let mut stmt = db
        .prepare(
            "select * from history left join places on places.id=history.place_id
    left join commands on history.command_id=commands.id",
        )
        .map_err(Error::PrepareSqliteQuery)?;

    let entries = stmt
        .query_map(params![], |row| {
            Ok(DBEntry {
                session: row.get(1)?,
                exit_status: row.get(4)?,
                start_time: row.get(5)?,
                duration: row.get(6)?,
                hostname: row.get(8)?,
                pwd: row.get(9)?,
                command: row.get(11)?,
            })
        })
        .map_err(Error::ConvertSqliteRow)?
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(Error::CollectEntries)?;

    info!("importing {:?} entries", entries.len());

    let mut session_ids = std::collections::HashMap::new();

    let store = crate::store::new(data_dir);

    for entry in entries {
        if entry.duration.is_none()
            || entry.exit_status.is_none()
            || entry.command.trim().is_empty()
        {
            continue;
        }

        let session_id = session_ids
            .entry((entry.session, entry.hostname.clone()))
            .or_insert_with(Uuid::new_v4);

        let start_time = entry.start_time;

        let time_start = chrono::DateTime::<Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(start_time, 0),
            Utc,
        );

        let time_finished = chrono::DateTime::<Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(
                start_time
                    + entry
                        .duration
                        .expect("save as we already checked if duration is some earlier"),
                0,
            ),
            Utc,
        );

        let hostname = entry.hostname;
        let pwd = PathBuf::from(entry.pwd);
        let result = entry
            .exit_status
            .expect("save as we already checked if status is some earlier")
            .try_into()
            .map_err(Error::ConvertExitStatus)?;

        let user = String::new();
        let command = entry.command;

        let entry = crate::entry::Entry {
            time_finished,
            time_start,
            hostname,
            pwd,
            result,
            session_id: *session_id,
            user,
            command,
        };

        store.add_entry(&entry)?;
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn histfile(import_file: impl AsRef<Path>, data_dir: PathBuf) -> Result<(), Error> {
    #[derive(Debug)]
    struct HistfileEntry {
        time_finished: DateTime<Utc>,
        result: u16,
        command: String,
    }

    let histfile = std::fs::File::open(import_file).map_err(Error::OpenHistfile)?;
    let reader = std::io::BufReader::new(histfile);

    let mut acc_time_finished: Option<DateTime<Utc>> = None;
    let mut acc_result: Option<u16> = None;
    let mut acc_command: Option<String> = None;
    let mut multiline_command = false;

    let mut entries = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;

        let line = match line {
            Err(err) => {
                warn!("can not read line {}: {}", line_number, err);

                continue;
            }
            Ok(line) => line,
        };

        // End of multiline command
        if line.starts_with(':') && multiline_command {
            let time_finished = acc_time_finished.ok_or(Error::TimeFinishedAccumulatorNone)?;
            let result = acc_result.ok_or(Error::ResultAccumulatorNone)?;
            let command = acc_command.ok_or(Error::CommandAccumulatorNone)?;

            acc_time_finished = None;
            acc_result = None;
            acc_command = None;
            multiline_command = false;

            entries.push(HistfileEntry {
                time_finished,
                result,
                command,
            });
        }

        if line.starts_with(':') {
            let mut split = line.split(':');

            let timestamp = split.nth(1).ok_or(Error::NoTimestamp(line_number))?.trim();

            let code_command = split.collect::<Vec<_>>().join(":");
            let mut code_command = code_command.split(';');

            let code = code_command.next().ok_or(Error::NoCode(line_number))?;

            let command = code_command.collect::<Vec<_>>().join(";");

            let time_finished = chrono::DateTime::<Utc>::from_utc(
                chrono::NaiveDateTime::from_timestamp(
                    timestamp
                        .parse()
                        .map_err(|err| Error::ParseTimestamp(err, line_number))?,
                    0,
                ),
                Utc,
            );

            let result = code
                .parse()
                .map_err(|err| Error::ParseResultCode(err, line_number))?;

            if command.ends_with('\\') {
                acc_time_finished = Some(time_finished);
                acc_result = Some(result);
                acc_command = Some(format!("{}\n", command.trim_end_matches('\\')));
                multiline_command = true;
            } else {
                entries.push(HistfileEntry {
                    time_finished,
                    result,
                    command,
                });
            }
        } else if let Some(ref mut acc) = acc_command {
            acc.push_str(&line);
            acc.push('\n');
        } else {
            unreachable!("line not starting with : and no multiline command");
        }
    }

    if acc_command.is_some() {
        let time_finished = acc_time_finished.expect("shoudnt fail if command is some");
        let result = acc_result.expect("shoudnt fail if command is some");
        let command = acc_command.expect("shoudnt fail if command is some");

        entries.push(HistfileEntry {
            time_finished,
            result,
            command,
        });
    }

    let store = crate::store::new(data_dir);

    let hostname = hostname::get()
        .map_err(Error::GetHostname)?
        .to_string_lossy()
        .to_string();

    let base_dirs = directories::BaseDirs::new().ok_or(Error::BaseDirectory)?;
    let pwd = base_dirs.home_dir().to_path_buf();
    let user = std::env::var("USER").map_err(Error::GetUser)?;
    let session_id = Uuid::new_v4();

    for histfile_entry in entries {
        let time_finished = histfile_entry.time_finished;
        let time_start = histfile_entry.time_finished;
        let result = histfile_entry.result;
        let command = histfile_entry.command;
        let hostname = hostname.clone();
        let pwd = pwd.clone();
        let user = user.clone();

        let entry = crate::entry::Entry {
            time_finished,
            time_start,
            hostname,
            command,
            pwd,
            result,
            session_id,
            user,
        };

        store.add_entry(&entry)?;
    }

    Ok(())
}
