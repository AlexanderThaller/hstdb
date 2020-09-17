mod client;
mod entry;
mod message;
mod server;
mod store;

use anyhow::{
    anyhow,
    Result,
};
use message::{
    CommandFinished,
    CommandStart,
    Message,
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

        "server" => {
            server::new().start()?;

            Ok(())
        }

        _ => Err(anyhow!("unkown {}", command)),
    }
}
