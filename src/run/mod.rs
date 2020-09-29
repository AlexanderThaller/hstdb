pub mod import;

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
use regex::Regex;
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
    Server(#[from] server::Error),

    #[error("{0}")]
    Store(#[from] store::Error),

    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[error("can not get base directories")]
    GetBaseDirectories,

    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),

    #[error("can not convert chrono milliseconds: {0}")]
    ConvertDuration(std::num::TryFromIntError),

    #[error("can not write to stdout: {0}")]
    WriteStdout(std::io::Error),

    #[error("can not import entries: {0}")]
    Import(import::Error),
}

#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
pub fn default(
    in_current: bool,
    folder: Option<PathBuf>,
    all_hosts: bool,
    hostname: Option<String>,
    data_dir: PathBuf,
    entries_count: usize,
    command: &Option<String>,
    no_subdirs: bool,
    command_text: &Option<Regex>,
    no_format: bool,
    host: bool,
    duration: bool,
    status: bool,
    show_pwd: bool,
    show_session: bool,
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
        command,
        &dir_filter,
        no_subdirs,
        command_text,
    )?;

    if no_format {
        let mut header = vec!["tmn"];

        if host {
            header.push("host")
        };

        if duration {
            header.push("duration")
        };

        if status {
            header.push("res")
        };

        if show_session {
            header.push("ses");
        }

        if show_pwd {
            header.push("pwd");
        }

        header.push("cmd");

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();

        handle
            .write_all(header.join("\t").as_bytes())
            .map_err(Error::WriteStdout)?;
        handle.write_all(b"\n").map_err(Error::WriteStdout)?;

        for entry in entries {
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

            if show_session {
                row.push(format_uuid(entry.session_id));
            }
            if show_pwd {
                row.push(format_pwd(&entry.pwd)?);
            }

            row.push(format_command(&entry.command, no_format));

            handle
                .write_all(row.join("\t").as_bytes())
                .map_err(Error::WriteStdout)?;
            handle.write_all(b"\n").map_err(Error::WriteStdout)?;
        }
    } else {
        let mut table = Table::new();
        table.load_preset("                   ");
        table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

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

        if show_session {
            header.push(Cell::new("ses").add_attribute(Attribute::Bold));
        }

        if show_pwd {
            header.push(Cell::new("pwd").add_attribute(Attribute::Bold));
        }

        header.push(Cell::new("cmd").add_attribute(Attribute::Bold));

        table.set_header(header);

        for entry in entries {
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

            if show_session {
                row.push(format_uuid(entry.session_id));
            }
            if show_pwd {
                row.push(format_pwd(&entry.pwd)?);
            }

            row.push(format_command(&entry.command, no_format));

            table.add_row(row);
        }

        println!("{}", table);
    }

    Ok(())
}

pub fn zsh_add_history(command: String, socket_path: PathBuf) -> Result<(), Error> {
    let data = CommandStart::from_env(command)?;

    client::new(socket_path).send(&Message::CommandStart(data))?;

    Ok(())
}

pub fn server(cache_path: PathBuf, socket_path: &PathBuf, data_dir: PathBuf) -> Result<(), Error> {
    let server = server::new(cache_path, data_dir)?;

    server.start(socket_path)?;

    Ok(())
}

pub fn stop(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(&Message::Stop)?;

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

pub fn running(socket_path: PathBuf) -> Result<(), Error> {
    client::new(socket_path).send(&Message::Running)?;

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

fn format_command(command: &str, no_format: bool) -> String {
    if no_format {
        command.trim().replace("\n", "\\n")
    } else {
        command.trim().to_string()
    }
}