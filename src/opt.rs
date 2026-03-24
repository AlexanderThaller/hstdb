use std::path::PathBuf;

use clap::{
    CommandFactory,
    Parser,
    Subcommand,
};
use color_eyre::eyre::WrapErr;
use directories::{
    BaseDirs,
    ProjectDirs,
};
use regex::Regex;
use thiserror::Error;

use crate::{
    config,
    run,
    run::{
        Display,
        TableDisplay,
    },
    store::Filter,
};

/// Errors returned while resolving platform-specific default directories.
#[derive(Error, Debug)]
pub enum Error {
    /// No suitable base directory could be resolved on the current platform.
    #[error("can not get base directories")]
    BaseDirectory,

    /// No suitable project directory could be resolved on the current platform.
    #[error("can not get project dirs")]
    ProjectDirs,
}

fn project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "hstdb", "hstdb")
        .ok_or(Error::ProjectDirs)
        .expect("can not get project_dir")
}

fn base_directory() -> BaseDirs {
    directories::BaseDirs::new()
        .ok_or(Error::BaseDirectory)
        .expect("can not get base directory")
}

fn default_data_dir() -> PathBuf {
    let project_dir = project_dir();
    let data_dir = project_dir.data_dir();

    data_dir.to_owned()
}

fn default_cache_path() -> PathBuf {
    let project_dir = project_dir();
    project_dir.cache_dir().join("server")
}

fn default_histdb_sqlite_path() -> PathBuf {
    std::env::var_os("HISTDB_FILE").map_or_else(
        || {
            let base_dirs = base_directory();
            let home = base_dirs.home_dir();
            home.join(".histdb").join("zsh-history.db")
        },
        PathBuf::from,
    )
}

fn default_zsh_histfile_path() -> PathBuf {
    std::env::var_os("HISTFILE").map_or_else(
        || {
            let base_dirs = base_directory();
            let home = base_dirs.home_dir();
            home.join(".histfile")
        },
        PathBuf::from,
    )
}

fn default_socket_path() -> PathBuf {
    let project_dir = project_dir();

    let fallback_path = PathBuf::from("/tmp/hstdb/");

    project_dir
        .runtime_dir()
        .unwrap_or(&fallback_path)
        .join("server_socket")
}

fn default_config_path() -> PathBuf {
    let project_dir = project_dir();

    project_dir.config_dir().join("config.toml")
}

#[derive(Parser, Debug)]
struct ZSHAddHistory {
    #[clap(flatten)]
    socket_path: Socket,

    /// Command to add to history
    #[clap(index = 1)]
    command: String,
}

#[derive(Parser, Debug)]
struct Server {
    /// Path to the cachefile used to store entries between restarts
    #[clap(short, long, default_value_os_t = default_cache_path())]
    cache_path: PathBuf,

    #[clap(flatten)]
    data_dir: DataDir,

    #[clap(flatten)]
    socket_path: Socket,
}

#[derive(Subcommand, Debug)]
enum Import {
    #[cfg(feature = "histdb-import")]
    /// Import entries from existing histdb sqlite file
    Histdb(ImportHistdb),

    /// Import entries from existing zsh histfile
    Histfile(ImportHistfile),
}

#[derive(Parser, Debug)]
struct ImportHistdb {
    #[clap(flatten)]
    data_dir: DataDir,

    /// Path to the existing histdb sqlite file
    #[clap(short, long, default_value_os_t = default_histdb_sqlite_path())]
    import_file: PathBuf,
}

#[derive(Parser, Debug)]
struct ImportHistfile {
    #[clap(flatten)]
    data_dir: DataDir,

    /// Path to the existing zsh histfile file
    #[clap(short, long, default_value_os_t = default_zsh_histfile_path())]
    import_file: PathBuf,
}

#[derive(Parser, Debug)]
struct Socket {
    /// Path to the socket for communication with the server
    #[clap(short, long, env = "HSTDB_SOCKET_PATH", default_value_os_t = default_socket_path())]
    socket_path: PathBuf,
}

#[derive(Parser, Debug)]
struct Config {
    /// Path to the configuration file
    #[clap(long, env = "HSTDB_CONFIG_PATH", default_value_os_t = default_config_path())]
    config_path: PathBuf,
}

#[derive(Parser, Debug)]
struct DataDir {
    /// Path to folder in which to store the history files
    #[clap(
        short,
        long,
        env = "HSTDB_DATA_DIR",
        default_value_os_t = default_data_dir()
    )]
    data_dir: PathBuf,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "this is a cli app and its fine if there are a lot of bools"
)]
#[derive(Parser, Debug)]
struct DefaultArgs {
    #[clap(flatten)]
    data_dir: DataDir,

    /// How many entries to print
    #[clap(short, long, default_value = "25")]
    entries_count: usize,

    /// Only print entries beginning with the given command
    #[clap(short, long)]
    command: Option<String>,

    /// Only print entries containing the given regex
    #[clap(short = 't', long = "text")]
    command_text: Option<Regex>,

    /// Only print entries not containing the given regex
    #[clap(short = 'T', long = "text-excluded", alias = "text_excluded")]
    command_text_excluded: Option<Regex>,

    /// Only print entries that have been executed in the current directory
    #[clap(short, long = "in", conflicts_with = "folder")]
    in_current: bool,

    /// Only print entries that have been executed in the given directory
    #[clap(short, long)]
    folder: Option<PathBuf>,

    /// Exclude subdirectories when filtering by folder
    #[clap(long)]
    no_subdirs: bool,

    /// Filter by given hostname
    #[clap(long, conflicts_with = "all_hosts")]
    hostname: Option<String>,

    /// Filter by given session
    #[clap(long)]
    session: Option<Regex>,

    /// Print all hosts
    #[clap(long)]
    all_hosts: bool,

    /// Disable fancy formatting
    #[clap(long)]
    disable_formatting: bool,

    /// Print host column
    #[clap(long)]
    show_host: bool,

    /// Print returncode of command
    #[clap(long)]
    show_status: bool,

    /// Show how long the command ran
    #[clap(long)]
    show_duration: bool,

    /// Show directory in which the command was run
    #[clap(long)]
    show_pwd: bool,

    /// Show session id for command
    #[clap(long)]
    show_session: bool,

    /// Disable printing of header
    #[clap(long)]
    hide_header: bool,

    /// Filter out failed commands (return code not 0)
    #[clap(long)]
    filter_failed: bool,

    /// Find commands with the given return code
    #[clap(long)]
    find_status: Option<u16>,

    #[clap(flatten)]
    config: Config,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    /// Add new command for current session
    #[clap(name = "zshaddhistory")]
    ZSHAddHistory(ZSHAddHistory),

    /// Start the server
    #[clap(name = "server")]
    Server(Server),

    /// Stop the server
    #[clap(name = "stop")]
    Stop(Socket),

    /// Disable history recording for current session
    #[clap(name = "disable")]
    Disable(Socket),

    /// Enable history recording for current session
    #[clap(name = "enable")]
    Enable(Socket),

    /// Finish command for current session
    #[clap(name = "precmd")]
    PreCmd(Socket),

    /// Get new session id
    #[clap(name = "session_id")]
    SessionID,

    /// Import entries from existing histdb sqlite or zsh histfile
    #[clap(subcommand, name = "import")]
    Import(Import),

    /// Print out shell functions needed by histdb and set current session id
    #[clap(name = "init")]
    Init,

    /// Run benchmark against server
    #[clap(name = "bench")]
    Bench(Socket),

    /// Generate autocomplete files for shells
    #[clap(name = "completion")]
    Completion(CompletionOpts),
}

#[derive(Parser, Debug)]
/// Options for generating shell completion scripts.
pub struct CompletionOpts {
    /// For which shell to generate the autocomplete
    #[clap(value_parser, default_value = "zsh")]
    shell: clap_complete::Shell,
}

#[derive(Parser, Debug)]
#[clap(version, about)]
/// Top-level command-line options for the `hstdb` binary.
pub struct Opt {
    #[clap(flatten)]
    default_args: DefaultArgs,

    #[clap(subcommand)]
    sub_command: Option<SubCommand>,
}

impl Opt {
    /// Executes the selected `hstdb` command.
    #[expect(
        clippy::too_many_lines,
        reason = "this is the main entry point for the CLI and it's fine if it's a bit long"
    )]
    pub fn run(self) -> color_eyre::Result<()> {
        let sub_command = self.sub_command;
        let in_current = self.default_args.in_current;
        let folder = self.default_args.folder;
        let all_hosts = self.default_args.all_hosts;
        let hostname = self.default_args.hostname;
        let data_dir = self.default_args.data_dir.data_dir;
        let entries_count = self.default_args.entries_count;
        let command = self.default_args.command;
        let session_filter = self.default_args.session;
        let no_subdirs = self.default_args.no_subdirs;
        let command_text = self.default_args.command_text;
        let command_text_excluded = self.default_args.command_text_excluded;
        let filter_failed = self.default_args.filter_failed;
        let find_status = self.default_args.find_status;
        let config = config::Config::open(self.default_args.config.config_path)
            .wrap_err("can not read configuration file")?;

        let format = !self.default_args.disable_formatting;
        let duration = Display::should_show(self.default_args.show_duration);
        let header = Display::should_hide(self.default_args.hide_header);
        let host = Display::should_show(self.default_args.show_host);
        let pwd = Display::should_show(self.default_args.show_pwd);
        let session = Display::should_show(self.default_args.show_session);
        let status = Display::should_show(self.default_args.show_status);

        env_logger::init();

        match sub_command {
            None => {
                let filter = Filter::new(&config)
                    .directory(folder, in_current, no_subdirs)
                    .wrap_err("can not apply directory filters from CLI arguments")?
                    .hostname(hostname, all_hosts)
                    .wrap_err("can not apply hostname filters from CLI arguments")?
                    .count(entries_count)
                    .command(command, command_text, command_text_excluded)
                    .session(session_filter)
                    .filter_failed(filter_failed)
                    .find_status(find_status);

                let display = TableDisplay {
                    format,

                    duration,
                    header,
                    host,
                    pwd,
                    session,
                    status,
                };

                run::default(&filter, &display, &data_dir)
                    .wrap_err("can not render history entries")?;
            }
            Some(sub_command) => match sub_command {
                SubCommand::ZSHAddHistory(o) => {
                    run::zsh_add_history(&config, o.command, &o.socket_path.socket_path)
                        .wrap_err("can not record command start from zshaddhistory")?;
                }
                SubCommand::Server(o) => {
                    run::server(
                        &o.cache_path,
                        &o.socket_path.socket_path,
                        &o.data_dir.data_dir,
                    )
                    .wrap_err("can not start hstdb server")?;
                }
                SubCommand::Stop(o) => {
                    run::stop(&o.socket_path).wrap_err("can not stop hstdb server")?;
                }
                SubCommand::Disable(o) => {
                    run::disable(&o.socket_path)
                        .wrap_err("can not disable history recording for this session")?;
                }
                SubCommand::Enable(o) => {
                    run::enable(&o.socket_path)
                        .wrap_err("can not enable history recording for this session")?;
                }
                SubCommand::PreCmd(o) => {
                    run::precmd(&o.socket_path)
                        .wrap_err("can not record command completion from precmd")?;
                }
                SubCommand::SessionID => run::session_id(),
                SubCommand::Import(s) => match s {
                    #[cfg(feature = "histdb-import")]
                    Import::Histdb(o) => run::import::histdb(&o.import_file, o.data_dir.data_dir)
                        .wrap_err_with(|| {
                        format!(
                            "can not import from histdb file {}",
                            o.import_file.display()
                        )
                    })?,
                    Import::Histfile(o) => {
                        run::import::histfile(&o.import_file, o.data_dir.data_dir).wrap_err_with(
                            || format!("can not import from histfile {}", o.import_file.display()),
                        )?;
                    }
                },
                SubCommand::Init => run::init(),
                SubCommand::Bench(s) => {
                    run::bench(s.socket_path)
                        .wrap_err("can not run benchmark traffic against hstdb server")?;
                }
                SubCommand::Completion(o) => {
                    let mut cmd = Opt::command();
                    let name = cmd.get_name().to_string();

                    clap_complete::generate(o.shell, &mut cmd, name, &mut std::io::stdout());
                }
            },
        }

        Ok(())
    }
}
