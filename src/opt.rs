use crate::{
    run,
    run::{
        Display,
        TableDisplay,
    },
    store::Filter,
};
use directories::ProjectDirs;
use log::error;
use regex::Regex;
use std::path::PathBuf;
use structopt::{
    clap::AppSettings::{
        ColoredHelp,
        GlobalVersion,
        NextLineHelp,
        VersionlessSubcommands,
    },
    StructOpt,
};
use thiserror::Error;

macro_rules! into_str {
    ($x:expr) => {{
        structopt::lazy_static::lazy_static! {
            static ref DATA: String = $x.to_string();
        }
        DATA.as_str()
    }};
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not get base directories")]
    BaseDirectory,

    #[error("can not get project dirs")]
    ProjectDirs,
}

fn get_default_or_fail<T>(func: fn() -> Result<T, Error>) -> T {
    match func() {
        Ok(s) => s,
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    }
}

fn project_dir() -> Result<ProjectDirs, Error> {
    ProjectDirs::from("com", "histdb-rs", "histdb-rs").ok_or(Error::ProjectDirs)
}

fn default_data_dir() -> Result<String, Error> {
    let project_dir = project_dir()?;
    let data_dir = project_dir.data_dir();

    Ok(data_dir.to_string_lossy().to_string())
}

fn default_cache_path() -> Result<String, Error> {
    let project_dir = project_dir()?;
    let cache_path = project_dir.cache_dir().join("server");

    Ok(cache_path.to_string_lossy().to_string())
}

fn default_histdb_sqlite_path() -> Result<String, Error> {
    let base_dirs = directories::BaseDirs::new().ok_or(Error::BaseDirectory)?;

    let home = base_dirs.home_dir();
    let file_path = home.join(".histdb").join("zsh-history.db");

    Ok(file_path.to_string_lossy().to_string())
}

fn default_zsh_histfile_path() -> Result<String, Error> {
    let base_dirs = directories::BaseDirs::new().ok_or(Error::BaseDirectory)?;

    let home = base_dirs.home_dir();
    let file_path = home.join(".histfile");

    Ok(file_path.to_string_lossy().to_string())
}

fn default_socket_path() -> Result<String, Error> {
    let project_dir = project_dir();

    let fallback_path = PathBuf::from("/tmp/histdb-rs/");

    let socket_path = project_dir?
        .runtime_dir()
        .unwrap_or(&fallback_path)
        .join("server_socket");

    Ok(socket_path.to_string_lossy().to_string())
}

#[derive(StructOpt, Debug)]
struct ZSHAddHistory {
    #[structopt(flatten)]
    socket_path: Socket,

    /// Command to add to history
    #[structopt(index = 1)]
    command: String,
}

#[derive(StructOpt, Debug)]
struct Server {
    /// Path to the cachefile used to store entries between restarts
    #[structopt(short, long, default_value = into_str!(get_default_or_fail(default_cache_path)))]
    cache_path: PathBuf,

    #[structopt(flatten)]
    data_dir: DataDir,

    #[structopt(flatten)]
    socket_path: Socket,
}

#[derive(StructOpt, Debug)]
enum Import {
    #[cfg(feature = "histdb-import")]
    /// Import entries from existing histdb sqlite file
    Histdb(ImportHistdb),

    /// Import entries from existing zsh histfile
    Histfile(ImportHistfile),
}

#[derive(StructOpt, Debug)]
struct ImportHistdb {
    #[structopt(flatten)]
    data_dir: DataDir,

    /// Path to the existing histdb sqlite file
    #[structopt(short, long, default_value = into_str!(get_default_or_fail(default_histdb_sqlite_path)))]
    import_file: PathBuf,
}

#[derive(StructOpt, Debug)]
struct ImportHistfile {
    #[structopt(flatten)]
    data_dir: DataDir,

    /// Path to the existing zsh histfile file
    #[structopt(short, long, default_value = into_str!(get_default_or_fail(default_zsh_histfile_path)))]
    import_file: PathBuf,
}

#[derive(StructOpt, Debug)]
struct Socket {
    /// Path to the socket for communication with the server
    #[structopt(short, long, default_value = into_str!(get_default_or_fail(default_socket_path)))]
    socket_path: PathBuf,
}

#[derive(StructOpt, Debug)]
struct DataDir {
    /// Path to folder in which to store the history files
    #[structopt(
        short,
        long,
        default_value = into_str!(get_default_or_fail(default_data_dir))
    )]
    data_dir: PathBuf,
}

#[allow(clippy::struct_excessive_bools)]
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

    /// Only print entries containing the given regex
    #[structopt(short = "t", long = "text")]
    command_text: Option<Regex>,

    /// Only print entries that have been executed in the current directory
    #[structopt(short, long = "in", conflicts_with = "folder")]
    in_current: bool,

    /// Only print entries that have been executed in the given directory
    #[structopt(short, long, conflicts_with = "in_current")]
    folder: Option<PathBuf>,

    /// Exclude subdirectories when filtering by folder
    #[structopt(long)]
    no_subdirs: bool,

    /// Filter by given hostname
    #[structopt(long, conflicts_with = "all_hosts")]
    hostname: Option<String>,

    /// Print all hosts
    #[structopt(long, conflicts_with = "hostname")]
    all_hosts: bool,

    /// Disable fancy formatting
    #[structopt(long)]
    disable_formatting: bool,

    /// Print host column
    #[structopt(long)]
    show_host: bool,

    /// Print returncode of command
    #[structopt(long)]
    show_status: bool,

    /// Show how long the command ran
    #[structopt(long)]
    show_duration: bool,

    /// Show directory in which the command was run
    #[structopt(long)]
    show_pwd: bool,

    /// Show session id for command
    #[structopt(long)]
    show_session: bool,

    /// Disable printing of header
    #[structopt(long)]
    hide_header: bool,
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

    /// Disable history recording for current session
    #[structopt(name = "disable")]
    Disable(Socket),

    /// Enable history recording for current session
    #[structopt(name = "enable")]
    Enable(Socket),

    /// Finish command for current session
    #[structopt(name = "precmd")]
    PreCmd(Socket),

    /// Get new session id
    #[structopt(name = "session_id")]
    SessionID,

    /// Import entries from existing histdb sqlite or zsh histfile
    #[structopt(name = "import")]
    Import(Import),

    /// Print out shell functions needed by histdb and set current session id
    #[structopt(name = "init")]
    Init,

    /// Run benchmark against server
    #[structopt(name = "bench")]
    Bench(Socket),
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

impl Opt {
    pub fn run(self) -> Result<(), run::Error> {
        let sub_command = self.sub_command;
        let in_current = self.default_args.in_current;
        let folder = self.default_args.folder;
        let all_hosts = self.default_args.all_hosts;
        let hostname = self.default_args.hostname;
        let data_dir = self.default_args.data_dir.data_dir;
        let entries_count = self.default_args.entries_count;
        let command = self.default_args.command;
        let no_subdirs = self.default_args.no_subdirs;
        let command_text = self.default_args.command_text;

        let format = !self.default_args.disable_formatting;
        let duration = Display::should_show(self.default_args.show_duration);
        let header = Display::should_hide(self.default_args.hide_header);
        let host = Display::should_show(self.default_args.show_host);
        let pwd = Display::should_show(self.default_args.show_pwd);
        let session = Display::should_show(self.default_args.show_session);
        let status = Display::should_show(self.default_args.show_status);

        sub_command.map_or_else(
            || {
                let filter = Filter::default()
                    .directory(folder, in_current, no_subdirs)?
                    .hostname(hostname, all_hosts)?
                    .count(entries_count)
                    .command(command, command_text);

                let display = TableDisplay {
                    format,

                    duration,
                    header,
                    host,
                    pwd,
                    session,
                    status,
                };

                run::default(&filter, &display, data_dir)
            },
            |sub_command| match sub_command {
                SubCommand::ZSHAddHistory(o) => {
                    run::zsh_add_history(o.command, o.socket_path.socket_path)
                }
                SubCommand::Server(o) => {
                    run::server(o.cache_path, o.socket_path.socket_path, o.data_dir.data_dir)
                }
                SubCommand::Stop(o) => run::stop(o.socket_path),
                SubCommand::Disable(o) => run::disable(o.socket_path),
                SubCommand::Enable(o) => run::enable(o.socket_path),
                SubCommand::PreCmd(o) => run::precmd(o.socket_path),
                SubCommand::SessionID => run::session_id(),
                SubCommand::Import(s) => match s {
                    #[cfg(feature = "histdb-import")]
                    Import::Histdb(o) => run::import::histdb(&o.import_file, o.data_dir.data_dir)
                        .map_err(run::Error::Import),
                    Import::Histfile(o) => {
                        run::import::histfile(&o.import_file, o.data_dir.data_dir)
                            .map_err(run::Error::Import)
                    }
                },
                SubCommand::Init => run::init(),
                SubCommand::Bench(s) => run::bench(s.socket_path),
            },
        )
    }
}
