pub mod import;

use crate::{
    client,
    entry::Entry,
    message,
    message::{
        session_id_from_env,
        CommandFinished,
        CommandStart,
        Message,
    },
    server,
    store,
    store::{
        filter,
        Filter,
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
use std::{
    convert::TryInto,
    io::Write,
    path::PathBuf,
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
    Server(#[from] server::ServerError),

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

    #[error("can not import entries: {0}")]
    Import(import::Error),
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
        if b {
            Self::Hide
        } else {
            Self::Show
        }
    }

    pub const fn should_show(b: bool) -> Self {
        if b {
            Self::Show
        } else {
            Self::Hide
        }
    }
}

pub fn default(filter: &Filter, display: &TableDisplay, data_dir: PathBuf) -> Result<(), Error> {
    let entries = store::new(data_dir).get_entries(filter)?;

    if display.format {
        default_format(display, entries)
    } else {
        default_no_format(display, entries)
    }
}

pub fn default_no_format(display: &TableDisplay, entries: Vec<Entry>) -> Result<(), Error> {
    let mut header = vec!["tmn"];

    if display.host.is_show() {
        header.push("host")
    };

    if display.duration.is_show() {
        header.push("duration")
    };

    if display.status.is_show() {
        header.push("res")
    };

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
        let mut row = vec![format_timestamp(entry.time_finished)];

        if display.host.is_show() {
            row.push(entry.hostname)
        }

        if display.duration.is_show() {
            row.push(format_duration(entry.time_start, entry.time_finished)?)
        }

        if display.status.is_show() {
            row.push(format!("{}", entry.result))
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
    }

    Ok(())
}

pub fn default_format(display: &TableDisplay, entries: Vec<Entry>) -> Result<(), Error> {
    let mut table = Table::new();
    table.load_preset("                   ");
    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

    let mut header = vec![Cell::new("tmn").add_attribute(Attribute::Bold)];

    if display.host.is_show() {
        header.push(Cell::new("host").add_attribute(Attribute::Bold))
    };

    if display.duration.is_show() {
        header.push(Cell::new("duration").add_attribute(Attribute::Bold))
    };

    if display.status.is_show() {
        header.push(Cell::new("res").add_attribute(Attribute::Bold))
    };

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
        let mut row = vec![format_timestamp(entry.time_finished)];

        if display.host.is_show() {
            row.push(entry.hostname)
        }

        if display.duration.is_show() {
            row.push(format_duration(entry.time_start, entry.time_finished)?)
        }

        if display.status.is_show() {
            row.push(format!("{}", entry.result))
        }

        if display.session.is_show() {
            row.push(format_uuid(entry.session_id));
        }
        if display.pwd.is_show() {
            row.push(format_pwd(&entry.pwd)?);
        }

        row.push(format_command(&entry.command, display.format));

        table.add_row(row);
    }

    println!("{}", table);

    Ok(())
}

pub fn zsh_add_history(command: String, socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandStart::from_env(command)?;

    client::new(socket_path).send(&Message::CommandStart(data))?;

    Ok(())
}

pub fn server(cache_dir: PathBuf, socket: PathBuf, data_dir: PathBuf) -> Result<(), Error> {
    server::builder(cache_dir, data_dir, socket)
        .build()?
        .run()?;

    Ok(())
}

pub fn stop(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(&Message::Stop)?;

    Ok(())
}

pub fn disable(socket_path: PathBuf) -> Result<(), Error> {
    let session_id = session_id_from_env()?;
    client::new(socket_path).send(&Message::Disable(session_id))?;

    Ok(())
}

pub fn enable(socket_path: PathBuf) -> Result<(), Error> {
    let session_id = session_id_from_env()?;
    client::new(socket_path).send(&Message::Enable(session_id))?;

    Ok(())
}

pub fn precmd(socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandFinished::from_env()?;

    client::new(socket_path).send(&Message::CommandFinished(data))?;

    Ok(())
}

pub fn session_id() -> Result<(), Error> {
    println!("{}", Uuid::new_v4());

    Ok(())
}

pub fn init() -> Result<(), Error> {
    println!("{}", include_str!("../../resources/init.zsh"));

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

fn format_pwd(pwd: &PathBuf) -> Result<String, Error> {
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

fn format_command(command: &str, format: bool) -> String {
    if format {
        command.trim().to_string()
    } else {
        command.trim().replace("\n", "\\n")
    }
}
