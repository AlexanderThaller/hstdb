use crate::{
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    store,
};
use std::{
    collections::HashMap,
    os::unix::net::UnixDatagram,
};
use thiserror::Error;
use uuid::Uuid;

const BUFFER_SIZE: usize = 65_527;

#[derive(Error, Debug)]
pub enum Error {
    #[error("command for session already started")]
    SessionCommandAlreadyStarted,

    #[error("command for session not started yet")]
    SessionCommandNotStarted,

    #[error("server is stopping")]
    Stop,

    #[error("can not add to storeo: {0}")]
    AddStore(crate::store::Error),
}

impl Error {
    fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            _ => false,
        }
    }
}

pub struct Server {
    entries: HashMap<Uuid, CommandStart>,
}

pub fn new() -> Server {
    Server {
        entries: HashMap::new(),
    }
}

impl Server {
    pub fn start(mut self) -> Result<(), Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let socket_path = xdg_dirs.place_runtime_file("socket").unwrap();

        let socket = UnixDatagram::bind(&socket_path).unwrap();

        loop {
            match Self::receive(&socket) {
                Err(err) => eprintln!("{}", err),
                Ok(message) => {
                    if let Err(err) = self.process(message) {
                        if err.is_stop() {
                            break;
                        }

                        eprintln!("error encountered: {}", err)
                    }
                }
            }
        }

        std::fs::remove_file(&socket_path).unwrap();

        Ok(())
    }

    fn process(&mut self, message: Message) -> Result<(), Error> {
        match message {
            Message::Stop => Err(Error::Stop),
            Message::CommandStart(data) => self.command_start(data),
            Message::CommandFinished(data) => self.command_finished(data),
        }
    }

    fn receive(socket: &UnixDatagram) -> Result<Message, Error> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let (written, _) = socket.recv_from(&mut buffer).unwrap();

        let message = bincode::deserialize(&buffer[0..written]).unwrap();

        Ok(message)
    }

    fn command_start(&mut self, start: CommandStart) -> Result<(), Error> {
        if self.entries.contains_key(&start.session_id) {
            return Err(Error::SessionCommandAlreadyStarted);
        }

        self.entries.insert(start.session_id, start);

        Ok(())
    }

    fn command_finished(&mut self, finish: CommandFinished) -> Result<(), Error> {
        if !self.entries.contains_key(&finish.session_id) {
            return Err(Error::SessionCommandNotStarted);
        }

        let start = self
            .entries
            .remove(&finish.session_id)
            .expect("already tested if exists");

        let entry = Entry::from_messages(start, finish);

        store::new().add(entry).map_err(Error::AddStore)?;

        Ok(())
    }
}
