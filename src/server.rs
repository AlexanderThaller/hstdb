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
use log::{
    info,
    warn,
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

    #[error("can not open cache file: {0}")]
    OpenCacheFile(std::io::Error),

    #[error("can not deserialize cache entries: {0}")]
    DeserializeCacheEntries(serde_json::Error),

    #[error("can not bind to socket: {0}")]
    BindSocket(std::io::Error),

    #[error("can not remove socket: {0}")]
    RemoveSocket(std::io::Error),

    #[error("can not create cache file: {0}")]
    CreateCacheFile(std::io::Error),

    #[error("can not serialize cache entries: {0}")]
    SerializeCacheEntries(serde_json::Error),

    #[error("can not receive message from socket: {0}")]
    ReceiveFromSocket(std::io::Error),

    #[error("can not deserialize message: {0}")]
    DeserializeMessage(bincode::Error),

    #[error("no parent directory for socket path")]
    NoSocketPathParent,

    #[error("can not create socket parent directory: {0}")]
    CreateSocketPathParent(std::io::Error),
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
    let file = std::fs::File::open(&cache_path).map_err(Error::OpenCacheFile)?;
    let reader = std::io::BufReader::new(file);

    let entries = serde_json::from_reader(reader).map_err(Error::DeserializeCacheEntries)?;

    let store = store::new(data_dir);

    Ok(Server {
        entries,
        store,
        cache_path,
    })
}

impl Server {
    pub fn start(mut self, socket_path: PathBuf) -> Result<Self, Error> {
        let socket_path_parent = socket_path.parent().ok_or(Error::NoSocketPathParent)?;
        std::fs::create_dir_all(socket_path_parent).map_err(Error::CreateSocketPathParent)?;

        info!("starting server listening on path {:?}", socket_path);
        let socket = UnixDatagram::bind(&socket_path).map_err(Error::BindSocket)?;

        loop {
            match Self::receive(&socket) {
                Err(err) => warn!("{}", err),
                Ok(message) => match self.process(message) {
                    Ok(state) => {
                        if state.is_stop() {
                            break;
                        }
                    }

                    Err(err) => warn!("error encountered: {}", err),
                },
            }
        }

        std::fs::remove_file(&socket_path).map_err(Error::RemoveSocket)?;

        let file = std::fs::File::create(&self.cache_path).map_err(Error::CreateCacheFile)?;
        let writer = std::io::BufWriter::new(file);

        serde_json::to_writer(writer, &self.entries).map_err(Error::SerializeCacheEntries)?;

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
        let (written, _) = socket
            .recv_from(&mut buffer)
            .map_err(Error::ReceiveFromSocket)?;

        let message =
            bincode::deserialize(&buffer[0..written]).map_err(Error::DeserializeMessage)?;

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
            info!(
                "session_id={session_id}, command={command}",
                session_id = session_id,
                command = entry.command
            )
        });

        Ok(RunState::Continue)
    }
}
