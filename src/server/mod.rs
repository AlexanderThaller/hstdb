pub mod builder;
pub mod db;

use bincode::serde::Compat;
pub use builder::{
    Builder,
    Error as BuilderError,
};

use crate::{
    client,
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    store::Store,
};
use crossbeam_utils::sync::WaitGroup;
use db::Db;
use flume::{
    Receiver,
    Sender,
};
use log::{
    info,
    warn,
};
use std::{
    os::unix::net::UnixDatagram,
    path::{
        Path,
        PathBuf,
    },
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
    #[error("can not receive message from socket: {0}")]
    ReceiveFromSocket(std::io::Error),

    #[error("can not send received data to processing: {0}")]
    SendBuffer(flume::SendError<Vec<u8>>),

    #[error("can not deserialize message: {0}")]
    DeserializeMessage(bincode::error::DecodeError),

    #[error("can not receive data from channel: {0}")]
    ReceiveData(flume::RecvError),

    #[error("can not remove socket: {0}")]
    RemoveSocket(std::io::Error),

    #[error("can not setup ctrlc handler: {0}")]
    SetupCtrlHandler(ctrlc::Error),

    #[error("command for session already started")]
    SessionCommandAlreadyStarted,

    #[error("command for session not started yet")]
    SessionCommandNotStarted,

    #[error("can not check if key exists in db: {0}")]
    CheckContainsEntry(db::Error),

    #[error("can not check if session is disabled in db: {0}")]
    CheckDisabledSession(db::Error),

    #[error("not recording because session {0} is disabled")]
    DisabledSession(Uuid),

    #[error("can not add entry to db: {0}")]
    AddDbEntry(db::Error),

    #[error("can not remove entry from db: {0}")]
    RemoveDbEntry(db::Error),

    #[error("can not add to storeo: {0}")]
    AddStore(crate::store::Error),

    #[error("db error: {0}")]
    Db(#[from] db::Error),
}

pub struct Server {
    pub(super) db: Db,
    pub(super) socket: UnixDatagram,
    pub(super) socket_path: PathBuf,
    pub(super) store: Store,
    pub(super) stopping: Arc<AtomicBool>,
    pub(super) wait_group: WaitGroup,
    pub(super) handle_ctrlc: bool,
}

pub fn builder(
    cache_dir: PathBuf,
    data_dir: PathBuf,
    socket: PathBuf,
    handle_ctrlc: bool,
) -> Builder {
    Builder {
        cache_dir,
        data_dir,
        socket,
        handle_ctrlc,
    }
}

impl Server {
    pub fn run(self) -> Result<(), Error> {
        let data_sender = Self::start_processor(
            Arc::clone(&self.stopping),
            self.wait_group.clone(),
            self.db,
            self.store,
            self.socket_path.clone(),
        );

        Self::start_receiver(
            Arc::clone(&self.stopping),
            self.wait_group.clone(),
            self.socket,
            data_sender,
        );

        if self.handle_ctrlc {
            Self::ctrl_c_watcher(self.stopping, self.socket_path.clone())?;
        }

        info!("listening on {:?}", self.socket_path);

        self.wait_group.wait();

        std::fs::remove_file(&self.socket_path).map_err(Error::RemoveSocket)?;

        Ok(())
    }

    fn ctrl_c_watcher(stopping: Arc<AtomicBool>, socket_path: PathBuf) -> Result<(), Error> {
        ctrlc::set_handler(move || {
            stopping.store(true, Ordering::SeqCst);

            let client = client::new(socket_path.clone());
            if let Err(err) = client.send(&Message::Stop) {
                warn!("{}", err);
            }
        })
        .map_err(Error::SetupCtrlHandler)?;

        Ok(())
    }

    fn start_receiver(
        stopping: Arc<AtomicBool>,
        wait_group: WaitGroup,
        socket: UnixDatagram,
        data_sender: Sender<Vec<u8>>,
    ) {
        thread::spawn(move || {
            loop {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }

                if let Err(err) = Self::receive(&socket, &data_sender) {
                    warn!("{}", err);
                }
            }

            drop(wait_group);
        });
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

    fn start_processor(
        stopping: Arc<AtomicBool>,
        wait_group: WaitGroup,
        db: Db,
        store: Store,
        socket_path: PathBuf,
    ) -> Sender<Vec<u8>> {
        let (data_sender, data_receiver) = flume::bounded(10_000);

        thread::spawn(move || {
            loop {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }

                if let Err(err) =
                    Self::process(&stopping, &data_receiver, &db, &store, &socket_path)
                {
                    warn!("{}", err);
                }
            }

            while !data_receiver.is_empty() {
                if let Err(err) =
                    Self::process(&stopping, &data_receiver, &db, &store, &socket_path)
                {
                    warn!("{}", err);
                }
            }

            drop(wait_group);
        });

        data_sender
    }

    fn process(
        stopping: &Arc<AtomicBool>,
        data_receiver: &Receiver<Vec<u8>>,
        db: &Db,
        store: &Store,
        socket_path: impl AsRef<Path>,
    ) -> Result<(), Error> {
        let data = data_receiver.recv().map_err(Error::ReceiveData)?;
        let (message, _): (Compat<Message>, _) =
            bincode::decode_from_slice(&data, bincode::config::standard())
                .map_err(Error::DeserializeMessage)?;

        match message.0 {
            Message::Stop => {
                stopping.store(true, Ordering::SeqCst);

                let client = client::new(socket_path.as_ref().to_path_buf());
                if let Err(err) = client.send(&Message::Stop) {
                    warn!("{}", err);
                }

                Ok(())
            }
            Message::CommandStart(data) => Self::command_start(db, &data),
            Message::CommandFinished(data) => Self::command_finished(db, store, &data),
            Message::Disable(uuid) => Self::disable_session(db, &uuid),
            Message::Enable(uuid) => Self::enable_session(db, &uuid),
        }
    }

    fn command_start(db: &Db, data: &CommandStart) -> Result<(), Error> {
        if db
            .contains_entry(&data.session_id)
            .map_err(Error::CheckContainsEntry)?
        {
            return Err(Error::SessionCommandAlreadyStarted);
        }

        if db
            .is_session_disabled(&data.session_id)
            .map_err(Error::CheckDisabledSession)?
        {
            return Err(Error::DisabledSession(data.session_id));
        }

        db.add_entry(data).map_err(Error::AddDbEntry)?;

        Ok(())
    }

    fn command_finished(db: &Db, store: &Store, data: &CommandFinished) -> Result<(), Error> {
        if db
            .is_session_disabled(&data.session_id)
            .map_err(Error::CheckDisabledSession)?
        {
            return Err(Error::DisabledSession(data.session_id));
        }

        if !db
            .contains_entry(&data.session_id)
            .map_err(Error::CheckContainsEntry)?
        {
            return Err(Error::SessionCommandNotStarted);
        }

        let start = db
            .remove_entry(&data.session_id)
            .map_err(Error::RemoveDbEntry)?;

        let entry = Entry::from_messages(start, data);

        store.add(&entry).map_err(Error::AddStore)?;

        Ok(())
    }

    fn disable_session(db: &Db, uuid: &Uuid) -> Result<(), Error> {
        db.disable_session(uuid)?;

        Ok(())
    }

    fn enable_session(db: &Db, uuid: &Uuid) -> Result<(), Error> {
        db.enable_session(uuid)?;

        Ok(())
    }
}
