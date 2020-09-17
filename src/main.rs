mod entry;
mod server;

use anyhow::{
    anyhow,
    Result,
};
use entry::Entry;
use server::Server;
use std::os::unix::net::UnixDatagram;
use uuid::Uuid;

fn main() -> Result<()> {
    let command = std::env::args()
        .into_iter()
        .skip(1)
        .next()
        .unwrap_or_default();

    match command.as_str() {
        "zshaddhistory" => {
            eprintln!("zshaddhistory");

            let entry = Entry::from_env().unwrap();
            dbg!(&entry);

            let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
            let socket_path = xdg_dirs.find_runtime_file("socket").unwrap();

            dbg!(&socket_path);

            let socket = UnixDatagram::unbound().unwrap();
            socket.connect(socket_path).unwrap();
            socket.send(format!("{:?}", entry).as_bytes()).unwrap();

            Ok(())
        }

        "precmd" => {
            println!("precmd");

            let entry = Entry::from_env().unwrap();
            dbg!(&entry);

            Ok(())
        }

        "session_id" => {
            eprintln!("session_id");

            println!("{}", Uuid::new_v4());

            Ok(())
        }

        "server" => {
            eprintln!("server");

            Server::start()?;

            Ok(())
        }

        _ => Err(anyhow!("unkown {}", command)),
    }
}
