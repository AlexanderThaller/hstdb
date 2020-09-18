mod client;
mod entry;
mod message;
mod server;
mod store;

use anyhow::Result;
use chrono::{
    DateTime,
    Utc,
};
use message::{
    CommandFinished,
    CommandStart,
    Message,
};
use prettytable::{
    cell,
    format,
    row,
    Table,
};
use std::path::PathBuf;
use uuid::Uuid;

fn main() -> Result<()> {
    let command = std::env::args().into_iter().nth(1).unwrap_or_default();

    match command.as_str() {
        "zshaddhistory" => {
            let data = CommandStart::from_env()?;
            client::new().send(Message::CommandStart(data))?;

            Ok(())
        }

        "precmd" => {
            let data = CommandFinished::from_env()?;
            client::new().send(Message::CommandFinished(data))?;

            Ok(())
        }

        "session_id" => {
            println!("{}", Uuid::new_v4());

            Ok(())
        }

        "stop" => {
            client::new().send(Message::Stop)?;

            Ok(())
        }

        "server" => {
            server::new().start()?;

            Ok(())
        }

        _ => {
            let hostname = hostname::get()?.to_string_lossy().to_string();

            let entries = store::new().get_nth_entries(&hostname, 25)?;

            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_CLEAN);
            table.set_titles(row![b->"time", b->"session", b->"pwd", b->"command"]);

            for entry in entries.into_iter() {
                table.add_row(row![
                    format_timestamp(entry.time_finished),
                    format_uuid(entry.session_id),
                    format_pwd(entry.pwd),
                    format!("{}", entry.command),
                ]);
            }

            table.printstd();

            Ok(())
        }
    }
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    let today = Utc::now().date();

    if timestamp.date() == today {
        timestamp.format("%H:%M").to_string()
    } else {
        timestamp.date().format("%m/%d").to_string()
    }
}

fn format_uuid(uuid: Uuid) -> String {
    let chars = uuid.to_string().chars().collect::<Vec<_>>();

    vec![chars[0], chars[1], chars[3], chars[4]]
        .into_iter()
        .collect()
}

fn format_pwd(pwd: PathBuf) -> String {
    let home = std::env::var("HOME").unwrap();

    if pwd.starts_with(home) {
        let mut without_home = PathBuf::from("~");

        let pwd_components = pwd.components().into_iter();
        let pwd_components = pwd_components.skip(3);

        pwd_components.for_each(|component| without_home.push(component));

        without_home.to_string_lossy().to_string()
    } else {
        pwd.to_string_lossy().to_string()
    }
}
