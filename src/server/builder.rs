use super::{
    Server,
    db,
};
use crate::store;
use color_eyre::eyre::WrapErr;
use crossbeam_utils::sync::WaitGroup;
use std::{
    os::unix::net::UnixDatagram,
    path::PathBuf,
    sync::{
        Arc,
        atomic::AtomicBool,
    },
};
use thiserror::Error;

/// Errors returned while constructing a [`Server`](super::Server).
#[derive(Error, Debug)]
pub enum Error {
    /// The configured socket path has no parent directory.
    #[error("no parent directory for socket path")]
    NoSocketPathParent,

    /// Creating the socket parent directory failed.
    #[error("can not create socket parent directory: {0}")]
    CreateSocketPathParent(std::io::Error),

    /// Binding the Unix socket failed.
    #[error("can not bind to socket at path {0}: {1}")]
    BindSocket(PathBuf, std::io::Error),

    /// Initializing the transient server database failed.
    #[error("{0}")]
    Db(#[from] db::Error),
}

/// Builder for creating a configured [`Server`](super::Server).
#[derive(Debug)]
pub struct Builder {
    pub(super) state_dir: PathBuf,
    pub(super) data_dir: PathBuf,
    pub(super) socket: PathBuf,
    pub(super) handle_ctrlc: bool,
}

impl Builder {
    /// Opens the transient database, binds the socket, and returns a server.
    pub fn build(self) -> color_eyre::Result<Server> {
        let db = db::new(self.state_dir).wrap_err("Failed to initialize server database")?;

        let socket_path_parent = self.socket.parent().ok_or(Error::NoSocketPathParent)?;
        std::fs::create_dir_all(socket_path_parent).map_err(Error::CreateSocketPathParent)?;
        let socket = UnixDatagram::bind(&self.socket)
            .map_err(|err| Error::BindSocket(self.socket.clone(), err))?;

        let store = store::new(self.data_dir);

        let stopping = Arc::new(AtomicBool::new(false));
        let wait_group = WaitGroup::new();

        let handle_ctrlc = self.handle_ctrlc;

        Ok(Server {
            db,
            socket,
            socket_path: self.socket,
            store,
            stopping,
            wait_group,
            handle_ctrlc,
        })
    }
}
