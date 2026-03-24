//! Server-side socket handling and coordination of in-flight commands.

/// Builder types for constructing a [`Server`].
pub(crate) mod builder;
/// Embedded key-value database used for transient server state.
pub(crate) mod db;

#[cfg(test)]
mod tests;

pub(crate) use builder::Builder;

use std::{
    os::unix::net::UnixDatagram,
    path::{
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
    thread,
};

use color_eyre::eyre::{
    WrapErr,
    bail,
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
use thiserror::Error;
use uuid::Uuid;

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

const BUFFER_SIZE: usize = 16_384;

/// Errors returned while receiving, decoding, and processing server messages.
#[derive(Error, Debug)]
pub(crate) enum Error {
    /// Reading a datagram from the Unix socket failed.
    #[error("can not receive message from socket: {0}")]
    ReceiveFromSocket(std::io::Error),

    /// Forwarding received bytes to the processing thread failed.
    #[error("can not send received data to processing: {0}")]
    SendBuffer(flume::SendError<Vec<u8>>),

    /// Deserializing a received message failed.
    #[error("can not deserialize message: {0}")]
    DeserializeMessage(bitcode::Error),

    /// Receiving queued work in the processing thread failed.
    #[error("can not receive data from channel: {0}")]
    ReceiveData(flume::RecvError),

    /// Removing the socket file during shutdown failed.
    #[error("can not remove socket: {0}")]
    RemoveSocket(std::io::Error),

    /// Installing the `Ctrl+C` handler failed.
    #[error("can not setup ctrlc handler: {0}")]
    SetupCtrlHandler(ctrlc::Error),

    /// A command start was received for a session that already has one in
    /// flight.
    #[error("command for session already started")]
    SessionCommandAlreadyStarted,

    /// A command finish was received without a matching start event.
    #[error("command for session not started yet")]
    SessionCommandNotStarted,

    /// Recording was skipped because the session is currently disabled.
    #[error("not recording because session {0} is disabled")]
    DisabledSession(Uuid),

    /// Persisting a finished entry to the store failed.
    #[error("can not add to store: {0}")]
    AddStore(crate::store::Error),
}

/// Running `hstdb` server instance.
#[derive(Debug)]
pub(crate) struct Server {
    pub(super) db: Db,
    pub(super) socket: UnixDatagram,
    pub(super) socket_path: PathBuf,
    pub(super) store: Store,
    pub(super) stopping: Arc<AtomicBool>,
    pub(super) wait_group: WaitGroup,
    pub(super) handle_ctrlc: bool,
}

#[must_use]
/// Creates a [`Builder`] for a server bound to the given paths.
pub(crate) fn builder(
    data_dir: PathBuf,
    state_dir: PathBuf,
    socket: PathBuf,
    handle_ctrlc: bool,
) -> Builder {
    Builder {
        state_dir,
        data_dir,
        socket,
        handle_ctrlc,
    }
}

impl Server {
    /// Starts the receiver and processor threads and blocks until shutdown.
    pub(crate) fn run(self) -> color_eyre::Result<()> {
        let data_sender = Self::start_processor(
            Arc::clone(&self.stopping),
            self.wait_group.clone(),
            self.db.clone(),
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

        info!("listening on {}", self.socket_path.display());

        self.wait_group.wait();

        std::fs::remove_file(&self.socket_path).map_err(Error::RemoveSocket)?;

        self.db
            .persist()
            .wrap_err("Failed to persist server database")?;

        Ok(())
    }

    fn ctrl_c_watcher(stopping: Arc<AtomicBool>, socket_path: PathBuf) -> Result<(), Error> {
        ctrlc::set_handler(move || {
            stopping.store(true, Ordering::SeqCst);

            let client = client::new(socket_path.clone());
            if let Err(err) = client.send(&Message::Stop) {
                warn!("{err}");
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
                    warn!("{err}");
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
                    warn!("{err}");
                }
            }

            while !data_receiver.is_empty() {
                if let Err(err) =
                    Self::process(&stopping, &data_receiver, &db, &store, &socket_path)
                {
                    warn!("{err}");
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
    ) -> color_eyre::Result<()> {
        let data = data_receiver.recv().map_err(Error::ReceiveData)?;
        let message = bitcode::deserialize(&data).map_err(Error::DeserializeMessage)?;

        match message {
            Message::Stop => {
                stopping.store(true, Ordering::SeqCst);

                let client = client::new(socket_path.as_ref().to_path_buf());
                if let Err(err) = client.send(&Message::Stop) {
                    warn!("{err}");
                }

                Ok(())
            }
            Message::CommandStart(data) => Self::command_start(db, &data),
            Message::CommandFinished(data) => Self::command_finished(db, store, &data),
            Message::Disable(uuid) => {
                Self::disable_session(db, &uuid);

                Ok(())
            }
            Message::Enable(uuid) => {
                Self::enable_session(db, &uuid);

                Ok(())
            }
        }
    }

    fn command_start(db: &Db, data: &CommandStart) -> color_eyre::Result<()> {
        if db.contains_entry(&data.session_id) {
            bail!(Error::SessionCommandAlreadyStarted)
        }

        if db.is_session_disabled(&data.session_id) {
            bail!(Error::DisabledSession(data.session_id))
        }

        db.add_entry(data);

        Ok(())
    }

    fn command_finished(db: &Db, store: &Store, data: &CommandFinished) -> color_eyre::Result<()> {
        if db.is_session_disabled(&data.session_id) {
            bail!(Error::DisabledSession(data.session_id))
        }

        if !db.contains_entry(&data.session_id) {
            bail!(Error::SessionCommandNotStarted)
        }

        let start = db
            .remove_entry(&data.session_id)
            .wrap_err("Failed to remove command start from db")?;

        let entry = Entry::from_messages(start, data);

        store.add(&entry).map_err(Error::AddStore)?;

        Ok(())
    }

    fn disable_session(db: &Db, uuid: &Uuid) {
        db.disable_session(uuid);
    }

    fn enable_session(db: &Db, uuid: &Uuid) {
        db.enable_session(uuid);
    }
}
