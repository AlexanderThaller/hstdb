use crate::message::Message;
use std::{
    os::unix::net::UnixDatagram,
    path::PathBuf,
};
use thiserror::Error;

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
    SerializeMessage(bincode::Error),

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

        socket
            .send(&bincode::serialize(&message).map_err(Error::SerializeMessage)?)
            .map_err(Error::SendMessage)?;

        Ok(())
    }
}
