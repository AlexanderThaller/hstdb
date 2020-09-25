use crate::{
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    store,
    store::Store,
};
use std::{
    collections::HashMap,
    os::unix::net::UnixDatagram,
    path::PathBuf,
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

#[derive(Debug)]
pub struct Server {
    entries: HashMap<Uuid, CommandStart>,
    store: Store,
    cache_path: PathBuf,
}

pub fn new(cache_path: PathBuf, data_dir: PathBuf) -> Result<Server, Error> {
    if cache_path.exists() {
        from_cachefile(cache_path, data_dir)
    } else {
        Ok(Server {
            entries: HashMap::new(),
            store: store::new(data_dir),
            cache_path,
        })
    }
}

fn from_cachefile(cache_path: PathBuf, data_dir: PathBuf) -> Result<Server, Error> {
    let file = std::fs::File::open(&cache_path).unwrap();
    let reader = std::io::BufReader::new(file);

    let entries = serde_json::from_reader(reader).unwrap();

    let store = store::new(data_dir);

    Ok(Server {
        entries,
        store,
        cache_path,
    })
}

impl Server {
    pub fn start(mut self, socket_path: PathBuf) -> Result<Self, Error> {
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

        let file = std::fs::File::create(&self.cache_path).unwrap();
        let writer = std::io::BufWriter::new(file);

        serde_json::to_writer(writer, &self.entries).unwrap();

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

        self.store.add(entry).map_err(Error::AddStore)?;

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
