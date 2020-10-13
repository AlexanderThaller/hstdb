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
use flume::{
    Receiver,
    Sender,
};
use log::{
    info,
    warn,
};
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    os::unix::net::UnixDatagram,
    path::PathBuf,
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
    thread,
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
    DeserializeState(serde_json::Error),

    #[error("can not bind to socket: {0}")]
    BindSocket(std::io::Error),

    #[error("can not remove socket: {0}")]
    RemoveSocket(std::io::Error),

    #[error("can not create cache file: {0}")]
    CreateCacheFile(std::io::Error),

    #[error("can not serialize cache entries: {0}")]
    SerializeState(serde_json::Error),

    #[error("can not receive message from socket: {0}")]
    ReceiveFromSocket(std::io::Error),

    #[error("no parent directory for socket path")]
    NoSocketPathParent,

    #[error("can not create socket parent directory: {0}")]
    CreateSocketPathParent(std::io::Error),

    #[error("not recording because session {0} is disabled")]
    DisabledSession(Uuid),

    #[error("can not send received data to processing: {0}")]
    SendBuffer(flume::SendError<Vec<u8>>),

    #[error("can not setup ctrlc handler: {0}")]
    SetupCtrlHandler(ctrlc::Error),
}

#[derive(Debug)]
enum RunState {
    Stop,
    Continue,
}

impl RunState {
    const fn is_stop(&self) -> bool {
        match self {
            Self::Stop => true,
            Self::Continue => false,
        }
    }
}

#[derive(Debug)]
pub struct Server {
    state: State,
    store: Store,
    cache_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    entries: HashMap<Uuid, CommandStart>,
    disabled_session: HashSet<Uuid>,
}

pub fn new(cache_path: PathBuf, data_dir: PathBuf) -> Result<Server, Error> {
    if cache_path.exists() {
        from_cachefile(cache_path, data_dir)
    } else {
        Ok(Server {
            state: State::default(),
            store: store::new(data_dir),
            cache_path,
        })
    }
}

fn from_cachefile(cache_path: PathBuf, data_dir: PathBuf) -> Result<Server, Error> {
    let file = std::fs::File::open(&cache_path).map_err(Error::OpenCacheFile)?;
    let reader = std::io::BufReader::new(file);

    let state = serde_json::from_reader(reader).map_err(Error::DeserializeState)?;

    let store = store::new(data_dir);

    Ok(Server {
        state,
        store,
        cache_path,
    })
}

impl Server {
    pub fn start(mut self, socket_path: &PathBuf) -> Result<Self, Error> {
        let socket_path_parent = socket_path.parent().ok_or(Error::NoSocketPathParent)?;
        std::fs::create_dir_all(socket_path_parent).map_err(Error::CreateSocketPathParent)?;

        info!("starting server listening on path {:?}", socket_path);
        let socket = UnixDatagram::bind(&socket_path).map_err(Error::BindSocket)?;

        let (data_sender, data_receiver) = flume::unbounded();
        let (signal_sender, signal_receiver) = flume::unbounded();
        let stopping = Arc::new(AtomicBool::new(false));

        Self::ctrl_c_watcher(signal_sender)?;
        Self::signal_watcher(signal_receiver, Arc::clone(&stopping));
        Self::start_processor(data_receiver);

        loop {
            if stopping.load(Ordering::SeqCst) {
                break;
            }

            if let Err(err) = Self::receive(&socket, &data_sender) {
                warn!("{}", err)
            }
        }

        std::fs::remove_file(&socket_path).map_err(Error::RemoveSocket)?;

        let file = std::fs::File::create(&self.cache_path).map_err(Error::CreateCacheFile)?;
        let writer = std::io::BufWriter::new(file);

        serde_json::to_writer(writer, &self.state).map_err(Error::SerializeState)?;

        Ok(self)
    }

    fn signal_watcher(signal_receiver: Receiver<RunState>, stopping: Arc<AtomicBool>) {
        thread::spawn(move || loop {
            match signal_receiver.recv() {
                Ok(signal) => match signal {
                    RunState::Stop => stopping.store(true, Ordering::SeqCst),
                    RunState::Continue => continue,
                },
                Err(err) => warn!("{}", err),
            }
        });
    }

    fn ctrl_c_watcher(signal_sender: Sender<RunState>) -> Result<(), Error> {
        ctrlc::set_handler(move || {
            if let Err(err) = signal_sender.send(RunState::Stop) {
                warn!("{}", err)
            }
        })
        .map_err(Error::SetupCtrlHandler)?;

        Ok(())
    }

    fn process(&mut self, message: Message) -> Result<RunState, Error> {
        match message {
            Message::Stop => Ok(RunState::Stop),
            Message::CommandStart(data) => self.command_start(data),
            Message::CommandFinished(data) => self.command_finished(&data),
            Message::Running => self.command_running(),
            Message::Disable(uuid) => self.disable_session(uuid),
            Message::Enable(uuid) => self.enable_session(uuid),
        }
    }

    fn receive(socket: &UnixDatagram, data_sender: &Sender<Vec<u8>>) -> Result<(), Error> {
        let mut buffer = [0_u8; BUFFER_SIZE];
        let (written, _) = socket
            .recv_from(&mut buffer)
            .map_err(Error::ReceiveFromSocket)?;

        data_sender
            .send(buffer[0..written].to_vec())
            .map_err(Error::SendBuffer)?;

        Ok(())
    }

    fn start_processor(data_receiver: Receiver<Vec<u8>>) -> Result<(), Error> {
        loop {
            let buffer = match data_receiver.recv() {
                Ok(b) => b,
                Err(err) => {
                    warn!("{}", err);
                    continue;
                }
            };

            let message = bincode::deserialize(&buffer).map_err(Error::DeserializeMessage)?;
        }
    }

    fn command_start(&mut self, start: CommandStart) -> Result<RunState, Error> {
        if self.state.entries.contains_key(&start.session_id) {
            return Err(Error::SessionCommandAlreadyStarted);
        }

        if self.state.disabled_session.contains(&start.session_id) {
            return Err(Error::DisabledSession(start.session_id));
        }

        self.state.entries.insert(start.session_id, start);

        Ok(RunState::Continue)
    }

    fn command_finished(&mut self, finish: &CommandFinished) -> Result<RunState, Error> {
        if self.state.disabled_session.contains(&finish.session_id) {
            return Err(Error::DisabledSession(finish.session_id));
        }

        if !self.state.entries.contains_key(&finish.session_id) {
            return Err(Error::SessionCommandNotStarted);
        }

        let start = self
            .state
            .entries
            .remove(&finish.session_id)
            .expect("already tested if exists");

        let entry = Entry::from_messages(start, finish);

        self.store.add(&entry).map_err(Error::AddStore)?;

        Ok(RunState::Continue)
    }

    fn command_running(&self) -> Result<RunState, Error> {
        self.state.entries.iter().for_each(|(session_id, entry)| {
            info!(
                "session_id={session_id}, command={command}",
                session_id = session_id,
                command = entry.command
            )
        });

        Ok(RunState::Continue)
    }

    fn disable_session(&mut self, session_id: Uuid) -> Result<RunState, Error> {
        self.state
            .entries
            .remove(&session_id)
            .expect("already tested if exists");

        self.state.disabled_session.insert(session_id);

        Ok(RunState::Continue)
    }

    fn enable_session(&mut self, session_id: Uuid) -> Result<RunState, Error> {
        self.state.disabled_session.remove(&session_id);
        Ok(RunState::Continue)
    }
}
