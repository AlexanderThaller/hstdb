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
    Local,
    Utc,
};
use comfy_table::{
    Attribute,
    Cell,
    Table,
};
use directories::ProjectDirs;
use log::info;
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

fn default_histdb_sqlite_path() -> String {
    let base_dirs = directories::BaseDirs::new().expect("getting basedirs should never fail");
    let home = base_dirs.home_dir();
    let file_path = home.join(".histdb").join("zsh-history.db");

    file_path.to_string_lossy().to_string()
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
    #[structopt(flatten)]
    data_dir: DataDir,

    #[structopt(flatten)]
    socket_path: Socket,

    /// Command to add to history
    #[structopt(index = 1)]
    command: String,
}

#[derive(StructOpt, Debug)]
struct Server {
    /// Path to the cachefile used to store entries between restarts
    #[structopt(short, long, default_value = into_str!(default_cache_path()))]
    cache_path: PathBuf,

    #[structopt(flatten)]
    data_dir: DataDir,

    #[structopt(flatten)]
    socket_path: Socket,
}

#[derive(StructOpt, Debug)]
struct Import {
    #[structopt(flatten)]
    data_dir: DataDir,

    /// Path to the existing histdb sqlite file
    #[structopt(short, long, default_value = into_str!(default_histdb_sqlite_path()))]
    import_file: PathBuf,
}

#[derive(StructOpt, Debug)]
struct Socket {
    /// Path to the socket for communication with the server
    #[structopt(short, long, default_value = into_str!(default_socket_path()))]
    socket_path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct DataDir {
    /// Path to folder in which to store the history files
    #[structopt(
        short,
        long,
        default_value = into_str!(default_data_dir())
    )]
    data_dir: PathBuf,
}

#[derive(StructOpt, Debug)]
struct DefaultArgs {
    #[structopt(flatten)]
    data_dir: DataDir,

    /// How many entries to print
    #[structopt(short, long, default_value = "25")]
    entries_count: usize,

    /// Only print entries beginning with the given command
    #[structopt(short, long)]
    command: Option<String>,

    /// Only print entries that have been executed in the current directory
    #[structopt(short, long = "in", conflicts_with = "folder")]
    in_current: bool,

    /// Only print entries that have been executed in the given directory
    #[structopt(short, long, conflicts_with = "in_current")]
    folder: Option<PathBuf>,

    /// Exclude subdirectories when filtering by folder
    #[structopt(long)]
    no_subdirs: bool,

    /// Print host column
    #[structopt(long)]
    host: bool,

    /// Filter by given hostname
    #[structopt(long, conflicts_with = "all_hosts")]
    hostname: Option<String>,

    /// Print all hosts
    #[structopt(long, conflicts_with = "hostname")]
    all_hosts: bool,

    /// Print returncode of command
    #[structopt(long)]
    status: bool,

    /// Show how long the command ran
    #[structopt(long)]
    duration: bool,

    /// Disable fancy formatting
    #[structopt(long)]
    no_format: bool,
}

#[derive(StructOpt, Debug)]
enum SubCommand {
    /// Add new command for current session
    #[structopt(name = "zshaddhistory")]
    ZSHAddHistory(ZSHAddHistory),

    /// Start the server
    #[structopt(name = "server")]
    Server(Server),

    /// Stop the server
    #[structopt(name = "stop")]
    Stop(Socket),

    /// Finish command for current session
    #[structopt(name = "precmd")]
    PreCmd(Socket),

    /// Get new session id
    #[structopt(name = "session_id")]
    SessionID,

    /// Tell server to print currently running command
    #[structopt(name = "running")]
    Running(Socket),

    /// Import entries from existing histdb sqlite file
    #[structopt(name = "import")]
    Import(Import),
}

#[derive(StructOpt, Debug)]
#[structopt(
    global_settings = &[ColoredHelp, VersionlessSubcommands, NextLineHelp, GlobalVersion]
)]
pub struct Opt {
    #[structopt(flatten)]
    default_args: DefaultArgs,

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

    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),

    #[error("can not convert chrono milliseconds: {0}")]
    ConvertDuration(std::num::TryFromIntError),
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
                    Self::run_zsh_add_history(o.command, o.socket_path.socket_path)
                }
                SubCommand::Server(o) => {
                    Self::run_server(o.cache_path, o.socket_path.socket_path, o.data_dir.data_dir)
                }
                SubCommand::Stop(o) => Self::run_stop(o.socket_path),
                SubCommand::PreCmd(o) => Self::run_precmd(o.socket_path),
                SubCommand::SessionID => Self::run_session_id(),
                SubCommand::Running(o) => Self::run_running(o.socket_path),
                SubCommand::Import(o) => Self::run_import(o.import_file, o.data_dir.data_dir),
            },

            None => Self::run_default(self.default_args),
        }
    }

    fn run_default(args: DefaultArgs) -> Result<(), Error> {
        let dir_filter = if args.in_current {
            Some(std::env::current_dir().map_err(Error::GetCurrentDir)?)
        } else {
            args.folder
        };

        let hostname = hostname::get()
            .map_err(Error::GetHostname)?
            .to_string_lossy()
            .to_string();

        let hostname_filter = if args.all_hosts {
            None
        } else {
            Some(args.hostname.unwrap_or(hostname))
        };

        let entries = store::new(args.data_dir.data_dir).get_entries(
            hostname_filter,
            args.entries_count,
            args.command,
            dir_filter,
            args.no_subdirs,
        )?;

        let mut table = Table::new();
        table.load_preset("                   ");
        table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
        if args.no_format {
            table.force_no_tty();
        }

        let mut header = vec![Cell::new("tmn").add_attribute(Attribute::Bold)];

        if args.host {
            header.push(Cell::new("host").add_attribute(Attribute::Bold))
        };

        if args.duration {
            header.push(Cell::new("duration").add_attribute(Attribute::Bold))
        };

        if args.status {
            header.push(Cell::new("res").add_attribute(Attribute::Bold))
        };

        header.push(Cell::new("ses").add_attribute(Attribute::Bold));
        header.push(Cell::new("pwd").add_attribute(Attribute::Bold));
        header.push(Cell::new("cmd").add_attribute(Attribute::Bold));

        table.set_header(header);

        for entry in entries.into_iter() {
            let mut row = vec![format_timestamp(entry.time_finished)];

            if args.host {
                row.push(entry.hostname)
            }

            if args.duration {
                row.push(format_duration(entry.time_start, entry.time_finished)?)
            }

            if args.status {
                row.push(format!("{}", entry.result))
            }

            row.push(format_uuid(entry.session_id));
            row.push(format_pwd(entry.pwd)?);
            row.push(format_command(entry.command, args.no_format));

            table.add_row(row);
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
