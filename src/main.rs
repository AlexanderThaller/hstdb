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
use comfy_table::{
    Attribute,
    Cell,
    Table,
};
use message::{
    CommandFinished,
    CommandStart,
    Message,
};
use rusqlite::params;
use std::{
    collections::{
        BTreeSet,
        HashMap,
    },
    convert::TryInto,
    path::PathBuf,
};
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

        "running" => {
            client::new().send(Message::Running)?;

            Ok(())
        }

        "import" => {
            let path = std::env::args().into_iter().nth(2).unwrap_or_default();

            dbg!(&path);

            let db = rusqlite::Connection::open(&path)?;
            let mut stmt = db.prepare(
                "select * from history left join places on places.id=history.place_id left join \
                 commands on history.command_id=commands.id",
            )?;

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
                })?
                .collect::<Result<BTreeSet<_>, _>>()?;

            println!("{:?}", entries.len());

            let mut session_ids: HashMap<(i64, String), Uuid> = HashMap::new();

            let store = crate::store::new();
            let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
            let datadir_path = xdg_dirs.get_data_home();

            for entry in entries {
                if entry.duration.is_none()
                    || entry.exit_status.is_none()
                    || entry.command.trim().is_empty()
                {
                    continue;
                }

                let session_id = session_ids
                    .entry((entry.session, entry.hostname.clone()))
                    .or_insert(Uuid::new_v4());

                let start_time = entry.start_time;

                let time_start = chrono::DateTime::<Utc>::from_utc(
                    chrono::NaiveDateTime::from_timestamp(start_time, 0),
                    Utc,
                );

                let time_finished = chrono::DateTime::<Utc>::from_utc(
                    chrono::NaiveDateTime::from_timestamp(start_time + entry.duration.unwrap(), 0),
                    Utc,
                );

                let hostname = entry.hostname;
                let pwd = PathBuf::from(entry.pwd);
                let result = entry.exit_status.unwrap().try_into().unwrap();
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

                store.add_entry(&entry, &datadir_path).unwrap();
            }

            let hostname = hostname::get()?.to_string_lossy().to_string();

            store.commit(&hostname)?;

            Ok(())
        }

        "server" => {
            let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs")?;

            let server = match xdg_dirs.find_cache_file("server.json") {
                None => server::new(),
                Some(path) => {
                    let file = std::fs::File::open(path).unwrap();
                    let reader = std::io::BufReader::new(file);

                    serde_json::from_reader(reader).unwrap()
                }
            };

            let server = server.start()?;

            let path = xdg_dirs.place_cache_file("server.json").unwrap();
            let file = std::fs::File::create(path).unwrap();
            let writer = std::io::BufWriter::new(file);

            serde_json::to_writer(writer, &server).unwrap();

            Ok(())
        }

        _ => {
            let hostname = hostname::get()?.to_string_lossy().to_string();

            let entries = store::new().get_nth_entries(Some(&hostname), 25)?;

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
                    format_pwd(entry.pwd),
                    entry.command.trim().to_string(),
                ]);
            }

            println!("{}", table);

            Ok(())
        }
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

fn format_pwd(pwd: PathBuf) -> String {
    let home = std::env::var("HOME").unwrap();

    if pwd.starts_with(home) {
        let mut without_home = PathBuf::from("~");

        let pwd_components = pwd.components().skip(3);

        pwd_components.for_each(|component| without_home.push(component));

        without_home.to_string_lossy().to_string()
    } else {
        pwd.to_string_lossy().to_string()
    }
}
