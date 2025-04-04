use super::{
    Server,
    db,
};
use crate::store;
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

#[derive(Error, Debug)]
pub enum Error {
    #[error("no parent directory for socket path")]
    NoSocketPathParent,

    #[error("can not create socket parent directory: {0}")]
    CreateSocketPathParent(std::io::Error),

    #[error("can not bind to socket: {0}")]
    BindSocket(std::io::Error),

    #[error("{0}")]
    Db(#[from] db::Error),
}

pub struct Builder {
    pub(super) cache_dir: PathBuf,
    pub(super) data_dir: PathBuf,
    pub(super) socket: PathBuf,
    pub(super) handle_ctrlc: bool,
}

impl Builder {
    pub fn build(self) -> Result<Server, Error> {
        let db = db::new(self.cache_dir)?;

        let socket_path_parent = self.socket.parent().ok_or(Error::NoSocketPathParent)?;
        std::fs::create_dir_all(socket_path_parent).map_err(Error::CreateSocketPathParent)?;
        let socket = UnixDatagram::bind(&self.socket).map_err(Error::BindSocket)?;

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
