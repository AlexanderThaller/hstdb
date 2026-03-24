//! High-level operations that back the `hstdb` CLI.

/// Import helpers for migrating existing shell history into `hstdb`.
pub mod import;
#[cfg(feature = "generate-readme")]
mod readme;

use crate::{
    client,
    config,
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
        session_id_from_env,
    },
    server,
    store,
    store::Filter,
};
use chrono::{
    DateTime,
    Local,
    Utc,
};
use color_eyre::eyre::WrapErr;
use comfy_table::{
    Attribute,
    Cell,
    Table,
};
use log::{
    debug,
    warn,
};
use std::{
    convert::TryInto,
    io::Write,
    path::{
        Path,
        PathBuf,
    },
};
use thiserror::Error;
use uuid::Uuid;

/// Errors returned by the top-level runtime entry points.
#[derive(Error, Debug)]
pub enum Error {
    /// No suitable base directory could be resolved on the current platform.
    #[error("can not get base directories")]
    GetBaseDirectories,

    /// Converting a signed duration into a standard duration failed.
    #[error("can not convert chrono milliseconds: {0}")]
    ConvertDuration(std::num::TryFromIntError),

    /// Writing unformatted output to stdout failed.
    #[error("can not write to stdout: {0}")]
    WriteStdout(std::io::Error),

    /// A command finish timestamp preceded its start timestamp.
    #[error("encountered negative duration when trying to format duration")]
    NegativeDuration,

    /// Formatting a specific entry for output failed.
    #[error("can not format entry: {0}\nentry: {1:?}")]
    FormatEntry(Box<Error>, Entry),

    #[cfg(feature = "generate-readme")]
    /// Regenerating the project README failed.
    #[error("{0}")]
    Readme(#[from] readme::Error),
}

/// Controls which columns are shown when rendering history output.
#[derive(Debug)]
pub struct TableDisplay {
    /// Chooses between table formatting and tab-separated output.
    pub format: bool,

    /// Controls whether the duration column is rendered.
    pub duration: Display,
    /// Controls whether the header row is rendered.
    pub header: Display,
    /// Controls whether the host column is rendered.
    pub host: Display,
    /// Controls whether the working-directory column is rendered.
    pub pwd: Display,
    /// Controls whether the session-id column is rendered.
    pub session: Display,
    /// Controls whether the exit-status column is rendered.
    pub status: Display,
}

impl Default for TableDisplay {
    fn default() -> Self {
        Self {
            format: true,

            duration: Display::Hide,
            header: Display::Show,
            host: Display::Hide,
            pwd: Display::Hide,
            session: Display::Hide,
            status: Display::Hide,
        }
    }
}

/// Simple visibility toggle used by [`TableDisplay`].
#[derive(Debug, Default)]
pub enum Display {
    /// Hide the associated field or column.
    #[default]
    Hide,
    /// Show the associated field or column.
    Show,
}

impl Display {
    const fn is_show(&self) -> bool {
        match self {
            Self::Hide => false,
            Self::Show => true,
        }
    }

    #[must_use]
    /// Returns [`Display::Hide`] when `b` is true, otherwise [`Display::Show`].
    pub const fn should_hide(b: bool) -> Self {
        if b { Self::Hide } else { Self::Show }
    }

    #[must_use]
    /// Returns [`Display::Show`] when `b` is true, otherwise [`Display::Hide`].
    pub const fn should_show(b: bool) -> Self {
        if b { Self::Show } else { Self::Hide }
    }
}

/// Loads entries from storage and prints them using the selected display mode.
pub fn default(
    filter: &Filter<'_>,
    display: &TableDisplay,
    data_dir: &Path,
) -> color_eyre::Result<()> {
    let entries = store::new(data_dir.to_path_buf())
        .get_entries(filter)
        .wrap_err_with(|| format!("can not load history entries from {}", data_dir.display()))?;

    if display.format {
        default_format(display, entries);

        Ok(())
    } else {
        default_no_format(display, entries)
    }
}

#[cfg(feature = "generate-readme")]
/// Regenerates `README.md` help sections from the clap command tree.
pub fn generate_readme(readme_path: PathBuf) -> color_eyre::Result<()> {
    readme::generate(readme_path).wrap_err("generating README file from clap help")?;
    Ok(())
}

/// Prints entries as tab-separated rows.
pub fn default_no_format(display: &TableDisplay, entries: Vec<Entry>) -> color_eyre::Result<()> {
    let mut header = vec!["tmn"];

    if display.host.is_show() {
        header.push("host");
    }

    if display.duration.is_show() {
        header.push("duration");
    }

    if display.status.is_show() {
        header.push("res");
    }

    if display.session.is_show() {
        header.push("ses");
    }

    if display.pwd.is_show() {
        header.push("pwd");
    }

    header.push("cmd");

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    if display.header.is_show() {
        handle
            .write_all(header.join("\t").as_bytes())
            .wrap_err("can not write history header to stdout")?;
        handle
            .write_all(b"\n")
            .wrap_err("can not terminate history header line on stdout")?;
    }

    for entry in entries {
        if let Err(err) = default_no_format_entry(&mut handle, display, &entry) {
            warn!("{}", Error::FormatEntry(Box::new(err), entry));
        }
    }

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
fn default_no_format_entry<T>(
    handle: &mut T,
    display: &TableDisplay,
    entry: &Entry,
) -> Result<(), Error>
where
    T: Write,
{
    let mut row = vec![format_timestamp(entry.time_finished)];

    if display.host.is_show() {
        row.push(entry.hostname.clone());
    }

    if display.duration.is_show() {
        row.push(format_duration(entry.time_start, entry.time_finished)?);
    }

    if display.status.is_show() {
        row.push(format!("{}", entry.result));
    }

    if display.session.is_show() {
        row.push(format_uuid(entry.session_id));
    }

    if display.pwd.is_show() {
        row.push(format_pwd(&entry.pwd)?);
    }

    row.push(format_command(&entry.command, display.format));

    handle
        .write_all(row.join("\t").as_bytes())
        .map_err(Error::WriteStdout)?;

    handle.write_all(b"\n").map_err(Error::WriteStdout)?;

    Ok(())
}

/// Prints entries using the formatted table renderer.
pub fn default_format(display: &TableDisplay, entries: Vec<Entry>) {
    let mut table = Table::new();
    table.load_preset("                   ");
    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

    let mut header = vec![Cell::new("tmn").add_attribute(Attribute::Bold)];

    if display.host.is_show() {
        header.push(Cell::new("host").add_attribute(Attribute::Bold));
    }

    if display.duration.is_show() {
        header.push(Cell::new("duration").add_attribute(Attribute::Bold));
    }

    if display.status.is_show() {
        header.push(Cell::new("res").add_attribute(Attribute::Bold));
    }

    if display.session.is_show() {
        header.push(Cell::new("ses").add_attribute(Attribute::Bold));
    }

    if display.pwd.is_show() {
        header.push(Cell::new("pwd").add_attribute(Attribute::Bold));
    }

    header.push(Cell::new("cmd").add_attribute(Attribute::Bold));

    if display.header.is_show() {
        table.set_header(header);
    }

    for entry in entries {
        if let Err(err) = default_format_entry(&mut table, display, &entry) {
            warn!("{}", Error::FormatEntry(Box::new(err), entry));
        }
    }

    println!("{table}");
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
fn default_format_entry(
    table: &mut Table,
    display: &TableDisplay,
    entry: &Entry,
) -> Result<(), Error> {
    let mut row = vec![format_timestamp(entry.time_finished)];

    if display.host.is_show() {
        row.push(entry.hostname.clone());
    }

    if display.duration.is_show() {
        row.push(format_duration(entry.time_start, entry.time_finished)?);
    }

    if display.status.is_show() {
        row.push(format!("{}", entry.result));
    }

    if display.session.is_show() {
        row.push(format_uuid(entry.session_id));
    }
    if display.pwd.is_show() {
        row.push(format_pwd(&entry.pwd)?);
    }

    row.push(format_command(&entry.command, display.format));

    table.add_row(row);

    Ok(())
}

/// Records a command start event emitted by the zsh `zshaddhistory` hook.
pub fn zsh_add_history(
    config: &config::Config,
    command: String,
    socket_path: &Path,
) -> color_eyre::Result<()> {
    if config.ignore_space && command.starts_with(' ') {
        debug!("not recording a command starting with a space");
    } else {
        let data = CommandStart::from_env(config, command)
            .wrap_err("can not build command-start message from current shell environment")?;
        client::new(socket_path.to_path_buf())
            .send(&Message::CommandStart(data))
            .wrap_err_with(|| {
                format!(
                    "can not send command-start message to hstdb socket {}",
                    socket_path.display()
                )
            })?;
    }

    Ok(())
}

/// Starts the local history server.
pub fn server(cache_dir: &Path, socket: &Path, data_dir: &Path) -> color_eyre::Result<()> {
    server::builder(
        cache_dir.to_path_buf(),
        data_dir.to_path_buf(),
        socket.to_path_buf(),
        true,
    )
    .build()
    .wrap_err_with(|| {
        format!(
            "can not build hstdb server with cache {}, data dir {}, and socket {}",
            cache_dir.display(),
            data_dir.display(),
            socket.display()
        )
    })?
    .run()
    .wrap_err_with(|| format!("can not run hstdb server on socket {}", socket.display()))?;

    Ok(())
}

/// Requests a graceful server shutdown over the control socket.
pub fn stop(socket_path: &Path) -> color_eyre::Result<()> {
    client::new(socket_path.to_path_buf())
        .send(&Message::Stop)
        .wrap_err_with(|| {
            format!(
                "can not send stop request to hstdb socket {}",
                socket_path.display()
            )
        })?;

    Ok(())
}

/// Disables history recording for the current session.
pub fn disable(socket_path: &Path) -> color_eyre::Result<()> {
    let session_id = session_id_from_env()
        .wrap_err("can not read session id from environment before disabling history")?;
    client::new(socket_path.to_path_buf())
        .send(&Message::Disable(session_id))
        .wrap_err_with(|| {
            format!(
                "can not send disable request to hstdb socket {}",
                socket_path.display()
            )
        })?;

    Ok(())
}

/// Re-enables history recording for the current session.
pub fn enable(socket_path: &Path) -> color_eyre::Result<()> {
    let session_id = session_id_from_env()
        .wrap_err("can not read session id from environment before enabling history")?;
    client::new(socket_path.to_path_buf())
        .send(&Message::Enable(session_id))
        .wrap_err_with(|| {
            format!(
                "can not send enable request to hstdb socket {}",
                socket_path.display()
            )
        })?;

    Ok(())
}

/// Records a command completion event emitted by the zsh `precmd` hook.
pub fn precmd(socket_path: &Path) -> color_eyre::Result<()> {
    let data = CommandFinished::from_env()
        .wrap_err("can not build command-finished message from current shell environment")?;

    client::new(socket_path.to_path_buf())
        .send(&Message::CommandFinished(data))
        .wrap_err_with(|| {
            format!(
                "can not send command-finished message to hstdb socket {}",
                socket_path.display()
            )
        })?;

    Ok(())
}

/// Prints a fresh session identifier to stdout.
pub fn session_id() {
    println!("{}", Uuid::new_v4());
}

/// Prints the bundled zsh initialization script to stdout.
pub fn init() {
    println!("{}", include_str!("../../resources/init.zsh"));
}

/// Continuously sends synthetic start and finish messages to the server.
pub fn bench(socket_path: PathBuf) -> color_eyre::Result<()> {
    let client = client::new(socket_path);

    let mut start = CommandStart {
        command: "test".to_string(),
        hostname: "test_hostname".to_string(),
        pwd: PathBuf::from("/tmp/test_pwd"),
        session_id: Uuid::new_v4(),
        time_stamp: Utc::now(),
        user: "test_user".to_string(),
    };

    let mut finished = CommandFinished {
        session_id: start.session_id,
        time_stamp: Utc::now(),
        result: 0,
    };

    loop {
        start.time_stamp = Utc::now();
        let message = Message::CommandStart(start.clone());

        client.send(&message).expect("ignore");

        finished.time_stamp = Utc::now();
        let message = Message::CommandFinished(finished.clone());

        client.send(&message).expect("ignore");
    }
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    let today = Local::now().date_naive();
    let local = timestamp.with_timezone(&chrono::offset::Local);
    let date = local.date_naive();

    if date == today {
        local.format("%H:%M").to_string()
    } else {
        local.date_naive().format("%Y-%m-%d").to_string()
    }
}

fn format_uuid(uuid: uuid::Uuid) -> String {
    let chars = uuid.to_string().chars().collect::<Vec<_>>();

    vec![chars[0], chars[1], chars[2], chars[3]]
        .into_iter()
        .collect()
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
fn format_pwd(pwd: impl AsRef<Path>) -> Result<String, Error> {
    let base_dirs = directories::BaseDirs::new().ok_or(Error::GetBaseDirectories)?;
    let home = base_dirs.home_dir();

    if pwd.as_ref().starts_with(home) {
        let mut without_home = PathBuf::from("~");

        let pwd_components = pwd.as_ref().components().skip(3);

        pwd_components.for_each(|component| without_home.push(component));

        Ok(without_home.to_string_lossy().to_string())
    } else {
        Ok(pwd.as_ref().to_string_lossy().to_string())
    }
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
fn format_duration(
    time_start: DateTime<Utc>,
    time_finished: DateTime<Utc>,
) -> Result<String, Error> {
    let duration = time_finished - time_start;
    let duration_ms = duration.num_milliseconds();

    if duration_ms < 0 {
        return Err(Error::NegativeDuration);
    }

    let duration_std =
        std::time::Duration::from_millis(duration_ms.try_into().map_err(Error::ConvertDuration)?);

    Ok(humantime::format_duration(duration_std)
        .to_string()
        .replace(' ', ""))
}

fn format_command(command: &str, format: bool) -> String {
    if format {
        command.trim().to_string()
    } else {
        command.trim().replace('\n', "\\n")
    }
}
