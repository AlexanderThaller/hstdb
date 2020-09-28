use crate::{
    client,
    message,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    server,
    store,
};
use chrono::{
    DateTime,
    Local,
    Utc,
};
use comfy_table::{
    Attribute,
    Cell,
    Table,
};
use log::info;
use regex::Regex;
use rusqlite::params;
use std::{
    convert::TryInto,
    path::PathBuf,
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    ClientError(#[from] client::Error),

    #[error("{0}")]
    MessageError(#[from] message::Error),

    #[error("{0}")]
    ServerError(#[from] server::Error),

    #[error("{0}")]
    StoreError(#[from] store::Error),

    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[error("can not open sqlite database: {0}")]
    OpenSqliteDatabase(rusqlite::Error),

    #[error("can not prepare sqlite query to get entries: {0}")]
    PrepareSqliteQuery(rusqlite::Error),

    #[error("can not convert sqlite row: {0}")]
    ConvertSqliteRow(rusqlite::Error),

    #[error("can not collect entries from sqlite query: {0}")]
    CollectEntries(rusqlite::Error),

    #[error("can not convert exit status from sqlite: {0}")]
    ConvertExitStatus(std::num::TryFromIntError),

    #[error("can not get base directories")]
    GetBaseDirectories,

    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),

    #[error("can not convert chrono milliseconds: {0}")]
    ConvertDuration(std::num::TryFromIntError),
}

pub fn default(
    in_current: bool,
    folder: Option<PathBuf>,
    all_hosts: bool,
    hostname: Option<String>,
    data_dir: PathBuf,
    entries_count: usize,
    command: Option<String>,
    no_subdirs: bool,
    command_text: Option<Regex>,
    no_format: bool,
    host: bool,
    duration: bool,
    status: bool,
) -> Result<(), Error> {
    let dir_filter = if in_current {
        Some(std::env::current_dir().map_err(Error::GetCurrentDir)?)
    } else {
        folder
    };

    let current_hostname = hostname::get()
        .map_err(Error::GetHostname)?
        .to_string_lossy()
        .to_string();

    let hostname_filter = if all_hosts {
        None
    } else {
        Some(hostname.unwrap_or(current_hostname))
    };

    let entries = store::new(data_dir).get_entries(
        hostname_filter,
        entries_count,
        &command,
        &dir_filter,
        no_subdirs,
        &command_text,
    )?;

    let mut table = Table::new();
    table.load_preset("                   ");
    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    if no_format {
        table.force_no_tty();
    }

    let mut header = vec![Cell::new("tmn").add_attribute(Attribute::Bold)];

    if host {
        header.push(Cell::new("host").add_attribute(Attribute::Bold))
    };

    if duration {
        header.push(Cell::new("duration").add_attribute(Attribute::Bold))
    };

    if status {
        header.push(Cell::new("res").add_attribute(Attribute::Bold))
    };

    header.push(Cell::new("ses").add_attribute(Attribute::Bold));
    header.push(Cell::new("pwd").add_attribute(Attribute::Bold));
    header.push(Cell::new("cmd").add_attribute(Attribute::Bold));

    table.set_header(header);

    for entry in entries.into_iter() {
        let mut row = vec![format_timestamp(entry.time_finished)];

        if host {
            row.push(entry.hostname)
        }

        if duration {
            row.push(format_duration(entry.time_start, entry.time_finished)?)
        }

        if status {
            row.push(format!("{}", entry.result))
        }

        row.push(format_uuid(entry.session_id));
        row.push(format_pwd(entry.pwd)?);
        row.push(format_command(entry.command, no_format));

        table.add_row(row);
    }

    println!("{}", table);

    Ok(())
}

pub fn zsh_add_history(command: String, socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandStart::from_env(command)?;

    client::new(socket_path).send(Message::CommandStart(data))?;

    Ok(())
}

pub fn server(cache_path: PathBuf, socket_path: PathBuf, data_dir: PathBuf) -> Result<(), Error> {
    let server = server::new(cache_path, data_dir)?;

    server.start(&socket_path)?;

    Ok(())
}

pub fn stop(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(Message::Stop)?;

    Ok(())
}

pub fn precmd(socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandFinished::from_env()?;

    client::new(socket_path).send(Message::CommandFinished(data))?;

    Ok(())
}

pub fn session_id() -> Result<(), Error> {
    println!("{}", Uuid::new_v4());

    Ok(())
}

pub fn running(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(Message::Running)?;

    Ok(())
}

pub fn import(import_file: PathBuf, data_dir: PathBuf) -> Result<(), Error> {
    let db = rusqlite::Connection::open(&import_file).map_err(Error::OpenSqliteDatabase)?;

    let mut stmt = db
        .prepare(
            "select * from history left join places on places.id=history.place_id
    left join commands on history.command_id=commands.id",
        )
        .map_err(Error::PrepareSqliteQuery)?;

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

    let hostname = hostname::get()
        .map_err(Error::GetHostname)?
        .to_string_lossy()
        .to_string();

    store.commit(format!("imported histdb file from {:?}", &hostname))?;

    Ok(())
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    let today = Local::now().date();
    let local = timestamp.with_timezone(&chrono::offset::Local);
    let date = local.date().with_timezone(&chrono::offset::Local);

    if date == today {
        local.format("%H:%M").to_string()
    } else {
        local.date().format("%Y-%m-%d").to_string()
    }
}

fn format_uuid(uuid: uuid::Uuid) -> String {
    let chars = uuid.to_string().chars().collect::<Vec<_>>();

    vec![chars[0], chars[1], chars[2], chars[3]]
        .into_iter()
        .collect()
}

fn format_pwd(pwd: PathBuf) -> Result<String, Error> {
    let base_dirs = directories::BaseDirs::new().ok_or(Error::GetBaseDirectories)?;
    let home = base_dirs.home_dir();

    if pwd.starts_with(home) {
        let mut without_home = PathBuf::from("~");

        let pwd_components = pwd.components().skip(3);

        pwd_components.for_each(|component| without_home.push(component));

        Ok(without_home.to_string_lossy().to_string())
    } else {
        Ok(pwd.to_string_lossy().to_string())
    }
}

fn format_duration(
    time_start: DateTime<Utc>,
    time_finished: DateTime<Utc>,
) -> Result<String, Error> {
    let duration = time_finished - time_start;
    let duration_ms = duration.num_milliseconds();
    let duration_std =
        std::time::Duration::from_millis(duration_ms.try_into().map_err(Error::ConvertDuration)?);

    Ok(humantime::format_duration(duration_std)
        .to_string()
        .replace(" ", ""))
}

fn format_command(command: String, no_format: bool) -> String {
    if no_format {
        command.trim().replace("\n", "\\n")
    } else {
        command.trim().to_string()
    }
}
