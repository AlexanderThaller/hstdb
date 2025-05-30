pub mod import;

use crate::{
    client,
    config,
    entry::Entry,
    message,
    message::{
        CommandFinished,
        CommandStart,
        Message,
        session_id_from_env,
    },
    server,
    store,
    store::{
        Filter,
        filter,
    },
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

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Client(#[from] client::Error),

    #[error("{0}")]
    Message(#[from] message::Error),

    #[error("{0}")]
    ServerBuilder(#[from] server::BuilderError),

    #[error("{0}")]
    Server(#[from] server::Error),

    #[error("{0}")]
    Store(#[from] store::Error),

    #[error("{0}")]
    Filter(#[from] filter::Error),

    #[error("can not get base directories")]
    GetBaseDirectories,

    #[error("can not convert chrono milliseconds: {0}")]
    ConvertDuration(std::num::TryFromIntError),

    #[error("can not write to stdout: {0}")]
    WriteStdout(std::io::Error),

    #[error("can not read configuration file: {0}")]
    ReadConfig(config::Error),

    #[error("encountered negative duration when trying to format duration")]
    NegativeDuration,

    #[cfg(feature = "histdb-import")]
    #[error("can not import from histdb: {0}")]
    ImportHistdb(import::Error),

    #[error("can not import from histfile: {0}")]
    ImportHistfile(import::Error),

    #[error("can not format entry: {0}\nentry: {1:?}")]
    FormatEntry(Box<Error>, Entry),
}

#[derive(Debug)]
pub struct TableDisplay {
    pub format: bool,

    pub duration: Display,
    pub header: Display,
    pub host: Display,
    pub pwd: Display,
    pub session: Display,
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

#[derive(Debug)]
pub enum Display {
    Hide,
    Show,
}

impl Default for Display {
    fn default() -> Self {
        Self::Hide
    }
}

impl Display {
    const fn is_show(&self) -> bool {
        match self {
            Self::Hide => false,
            Self::Show => true,
        }
    }

    pub const fn should_hide(b: bool) -> Self {
        if b { Self::Hide } else { Self::Show }
    }

    pub const fn should_show(b: bool) -> Self {
        if b { Self::Show } else { Self::Hide }
    }
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn default(filter: &Filter, display: &TableDisplay, data_dir: PathBuf) -> Result<(), Error> {
    let entries = store::new(data_dir).get_entries(filter)?;

    if display.format {
        default_format(display, entries);

        Ok(())
    } else {
        default_no_format(display, entries)
    }
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn default_no_format(display: &TableDisplay, entries: Vec<Entry>) -> Result<(), Error> {
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
            .map_err(Error::WriteStdout)?;

        handle.write_all(b"\n").map_err(Error::WriteStdout)?;
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

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn zsh_add_history(
    config: &config::Config,
    command: String,
    socket_path: PathBuf,
) -> Result<(), Error> {
    if config.ignore_space && command.starts_with(' ') {
        debug!("not recording a command starting with a space");
    } else {
        let data = CommandStart::from_env(config, command)?;
        client::new(socket_path).send(&Message::CommandStart(data))?;
    }

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn server(cache_dir: PathBuf, socket: PathBuf, data_dir: PathBuf) -> Result<(), Error> {
    server::builder(cache_dir, data_dir, socket, true)
        .build()?
        .run()?;

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn stop(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(&Message::Stop)?;

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn disable(socket_path: PathBuf) -> Result<(), Error> {
    let session_id = session_id_from_env()?;
    client::new(socket_path).send(&Message::Disable(session_id))?;

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn enable(socket_path: PathBuf) -> Result<(), Error> {
    let session_id = session_id_from_env()?;
    client::new(socket_path).send(&Message::Enable(session_id))?;

    Ok(())
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn precmd(socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandFinished::from_env()?;

    client::new(socket_path).send(&Message::CommandFinished(data))?;

    Ok(())
}

pub fn session_id() {
    println!("{}", Uuid::new_v4());
}

pub fn init() {
    println!("{}", include_str!("../../resources/init.zsh"));
}

#[expect(clippy::result_large_err, reason = "will fix this if needed")]
pub fn bench(socket_path: PathBuf) -> Result<(), Error> {
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
