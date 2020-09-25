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
use anyhow::Result;
use chrono::{
    DateTime,
    Utc,
};
use comfy_table::{
    Attribute,
    Cell,
    Table,
};
use directories::ProjectDirs;
use rusqlite::params;
use std::{
    convert::TryInto,
    path::PathBuf,
};
use structopt::{
    clap::AppSettings::*,
    StructOpt,
};
use thiserror::Error;
use uuid::Uuid;

macro_rules! into_str {
    ($x:expr) => {{
        structopt::lazy_static::lazy_static! {
            static ref DATA: String = $x.to_string();
        }
        DATA.as_str()
    }};
}

fn project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "histdb-rs", "histdb-rs")
        .expect("getting project dirs should never fail")
}

fn default_data_dir() -> String {
    let project_dir = project_dir();
    let data_dir = project_dir.data_dir();

    data_dir.to_string_lossy().to_string()
}

fn default_cache_path() -> String {
    let project_dir = project_dir();
    let cache_path = project_dir.cache_dir().join("server.json");

    cache_path.to_string_lossy().to_string()
}

fn default_socket_path() -> String {
    let project_dir = project_dir();
    let socket_path = project_dir
        .runtime_dir()
        // TODO: Sometimes getting the runtime dir can fail maybe find a good fallback path and use
        // that instead. Or find a good way to propagate the error to structopt.
        .expect("getting the runtime dir should never fail")
        .join("server_socket");

    socket_path.to_string_lossy().to_string()
}

#[derive(StructOpt, Debug)]
struct ZSHAddHistory {
    #[structopt(index = 1)]
    command: String,
}

#[derive(StructOpt, Debug)]
struct Server {
    #[structopt(short, long, default_value = into_str!(default_cache_path()))]
    cache_path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct Import {
    #[structopt(short, long, default_value = into_str!(default_cache_path()))]
    import_file: PathBuf,
}

#[derive(StructOpt, Debug)]
enum SubCommand {
    #[structopt(name = "zshaddhistory")]
    ZSHAddHistory(ZSHAddHistory),

    #[structopt(name = "server")]
    Server(Server),

    #[structopt(name = "stop")]
    Stop,

    #[structopt(name = "precmd")]
    PreCmd,

    #[structopt(name = "session_id")]
    SessionID,

    #[structopt(name = "running")]
    Running,

    #[structopt(name = "import")]
    Import(Import),
}

#[derive(StructOpt, Debug)]
#[structopt(
    global_settings = &[ColoredHelp, VersionlessSubcommands, NextLineHelp, GlobalVersion]
)]
pub struct Opt {
    /// Path to folder in which to store the history files.
    #[structopt(
        global = true,
        short,
        long,
        default_value = into_str!(default_data_dir())
    )]
    data_dir: PathBuf,

    /// Path to the socket for communication with the server
    #[structopt(global = true, short, long, default_value = into_str!(default_socket_path()))]
    socket_path: PathBuf,

    #[structopt(subcommand)]
    sub_command: Option<SubCommand>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    ClientError(client::Error),

    #[error("{0}")]
    MessageError(message::Error),

    #[error("{0}")]
    ServerError(server::Error),

    #[error("{0}")]
    StoreError(store::Error),

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
}

impl From<client::Error> for Error {
    fn from(err: client::Error) -> Self {
        Error::ClientError(err)
    }
}

impl From<message::Error> for Error {
    fn from(err: message::Error) -> Self {
        Error::MessageError(err)
    }
}

impl From<server::Error> for Error {
    fn from(err: server::Error) -> Self {
        Error::ServerError(err)
    }
}

impl From<store::Error> for Error {
    fn from(err: store::Error) -> Self {
        Error::StoreError(err)
    }
}

impl Opt {
    pub fn run(self) -> Result<(), Error> {
        let sub_command = self.sub_command;

        match sub_command {
            Some(sub_command) => match sub_command {
                SubCommand::ZSHAddHistory(o) => {
                    Self::run_zsh_add_history(o.command, self.socket_path)
                }
                SubCommand::Server(o) => {
                    Self::run_server(o.cache_path, self.socket_path, self.data_dir)
                }
                SubCommand::Stop => Self::run_stop(self.socket_path),
                SubCommand::PreCmd => Self::run_precmd(self.socket_path),
                SubCommand::SessionID => Self::run_session_id(),
                SubCommand::Running => Self::run_running(self.socket_path),
                SubCommand::Import(o) => Self::run_import(o.import_file, self.data_dir),
            },

            None => Self::run_default(self.data_dir),
        }
    }

    fn run_default(data_dir: PathBuf) -> Result<(), Error> {
        let hostname = hostname::get()
            .map_err(Error::GetHostname)?
            .to_string_lossy()
            .to_string();

        let entries = store::new(data_dir).get_nth_entries(Some(&hostname), 25)?;

        let mut table = Table::new();
        table.load_preset("                   ");
        table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
        table.set_header(vec![
            Cell::new("tmn").add_attribute(Attribute::Bold),
            Cell::new("ses").add_attribute(Attribute::Bold),
            Cell::new("res").add_attribute(Attribute::Bold),
            Cell::new("pwd").add_attribute(Attribute::Bold),
            Cell::new("cmd").add_attribute(Attribute::Bold),
        ]);

        for entry in entries.into_iter() {
            table.add_row(vec![
                format_timestamp(entry.time_finished),
                format_uuid(entry.session_id),
                format!("{}", entry.result),
                format_pwd(entry.pwd)?,
                entry.command.trim().to_string(),
            ]);
        }

        println!("{}", table);

        Ok(())
    }

    fn run_zsh_add_history(command: String, socket_path: PathBuf) -> Result<(), Error> {
        let data = CommandStart::from_env(command)?;

        client::new(socket_path).send(Message::CommandStart(data))?;

        Ok(())
    }

    fn run_server(
        cache_path: PathBuf,
        socket_path: PathBuf,
        data_dir: PathBuf,
    ) -> Result<(), Error> {
        let server = server::new(cache_path, data_dir)?;

        server.start(socket_path)?;

        Ok(())
    }

    fn run_stop(socket_path: PathBuf) -> Result<(), Error> {
        client::new(socket_path).send(Message::Stop)?;

        Ok(())
    }

    fn run_precmd(socket_path: PathBuf) -> Result<(), Error> {
        let data = CommandFinished::from_env()?;

        client::new(socket_path).send(Message::CommandFinished(data))?;

        Ok(())
    }

    fn run_session_id() -> Result<(), Error> {
        println!("{}", Uuid::new_v4());

        Ok(())
    }

    fn run_running(socket_path: PathBuf) -> Result<(), Error> {
        client::new(socket_path).send(Message::Running)?;

        Ok(())
    }

    fn run_import(import_file: PathBuf, data_dir: PathBuf) -> Result<(), Error> {
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

        println!("importing {:?} entries", entries.len());

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
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    let today = Utc::now().date();
    let date = timestamp.date();

    if date == today {
        timestamp.format("%H:%M").to_string()
    } else {
        timestamp.date().format("%Y-%m-%d").to_string()
    }
}

fn format_uuid(uuid: Uuid) -> String {
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
