use bincode::serde::BorrowCompat;
use std::{
    os::unix::net::UnixDatagram,
    path::PathBuf,
};
use thiserror::Error;

use crate::message::Message;

#[derive(Debug)]
pub struct Client {
    socket_path: PathBuf,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not create socket: {0}")]
    CreateSocket(std::io::Error),

    #[error("can not connect socket: {0}")]
    ConnectSocket(std::io::Error),

    #[error("can not serialize message: {0}")]
    SerializeMessage(bincode::error::EncodeError),

    #[error("can not send message to socket: {0}")]
    SendMessage(std::io::Error),
}

pub const fn new(socket_path: PathBuf) -> Client {
    Client { socket_path }
}

impl Client {
    pub fn send(&self, message: &Message) -> Result<(), Error> {
        let socket = UnixDatagram::unbound().map_err(Error::CreateSocket)?;

        socket
            .connect(&self.socket_path)
            .map_err(Error::ConnectSocket)?;

        let data = bincode::encode_to_vec(BorrowCompat(message), bincode::config::standard())
            .map_err(Error::SerializeMessage)?;

        socket.send(&data).map_err(Error::SendMessage)?;

        Ok(())
    }
}
