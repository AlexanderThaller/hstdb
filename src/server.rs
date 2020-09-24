use crate::{
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    store,
};
use serde::{
    Deserialize,
    Serialize,
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

    #[error("can not add to storeo: {0}")]
    AddStore(crate::store::Error),
}

#[derive(Debug)]
enum RunState {
    Stop,
    Continue,
}

impl RunState {
    fn is_stop(&self) -> bool {
        match self {
            RunState::Stop => true,
            RunState::Continue => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    entries: HashMap<Uuid, CommandStart>,
}

pub fn new() -> Server {
    Server {
        entries: HashMap::new(),
    }
}

impl Server {
    pub fn start(mut self) -> Result<Self, Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let socket_path = xdg_dirs.place_runtime_file("socket").unwrap();

        let socket = UnixDatagram::bind(&socket_path).unwrap();

        loop {
            match Self::receive(&socket) {
                Err(err) => eprintln!("{}", err),
                Ok(message) => match self.process(message) {
                    Ok(state) => {
                        if state.is_stop() {
                            break;
                        }
                    }

                    Err(err) => eprintln!("error encountered: {}", err),
                },
            }
        }

        std::fs::remove_file(&socket_path).unwrap();

        Ok(self)
    }

    fn process(&mut self, message: Message) -> Result<RunState, Error> {
        match message {
            Message::Stop => Ok(RunState::Stop),
            Message::CommandStart(data) => self.command_start(data),
            Message::CommandFinished(data) => self.command_finished(data),
            Message::Running => self.command_running(),
        }
    }

    fn receive(socket: &UnixDatagram) -> Result<Message, Error> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let (written, _) = socket.recv_from(&mut buffer).unwrap();

        let message = bincode::deserialize(&buffer[0..written]).unwrap();

        Ok(message)
    }

    fn command_start(&mut self, start: CommandStart) -> Result<RunState, Error> {
        if self.entries.contains_key(&start.session_id) {
            return Err(Error::SessionCommandAlreadyStarted);
        }

        self.entries.insert(start.session_id, start);

        Ok(RunState::Continue)
    }

    fn command_finished(&mut self, finish: CommandFinished) -> Result<RunState, Error> {
        if !self.entries.contains_key(&finish.session_id) {
            return Err(Error::SessionCommandNotStarted);
        }

        let start = self
            .entries
            .remove(&finish.session_id)
            .expect("already tested if exists");

        let entry = Entry::from_messages(start, finish);

        store::new().add(entry).map_err(Error::AddStore)?;

        Ok(RunState::Continue)
    }

    fn command_running(&self) -> Result<RunState, Error> {
        self.entries.iter().for_each(|(session_id, entry)| {
            println!(
                "session_id={session_id}, command={command}",
                session_id = session_id,
                command = entry.command
            )
        });

        Ok(RunState::Continue)
    }
}
