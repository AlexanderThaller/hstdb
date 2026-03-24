use std::{
    os::unix::net::UnixDatagram,
    path::PathBuf,
};
use thiserror::Error;

use crate::message::Message;

/// Sends serialized [`Message`] values to the local `hstdb` Unix socket.
#[derive(Debug)]
pub struct Client {
    socket_path: PathBuf,
}

/// Errors returned while sending a message to the server.
#[derive(Error, Debug)]
pub enum Error {
    /// Creating the local client socket failed.
    #[error("can not create socket: {0}")]
    CreateSocket(std::io::Error),

    /// Connecting the client socket to the server socket path failed.
    #[error("can not connect socket: {0}")]
    ConnectSocket(std::io::Error),

    /// Serializing the outgoing message failed.
    #[error("can not serialize message: {0}")]
    SerializeMessage(bitcode::Error),

    /// Writing the serialized message to the socket failed.
    #[error("can not send message to socket: {0}")]
    SendMessage(std::io::Error),
}

/// Creates a client that targets the given Unix domain socket path.
#[must_use]
pub const fn new(socket_path: PathBuf) -> Client {
    Client { socket_path }
}

impl Client {
    /// Serializes and sends a message to the configured server socket.
    pub fn send(&self, message: &Message) -> Result<(), Error> {
        let socket = UnixDatagram::unbound().map_err(Error::CreateSocket)?;

        socket
            .connect(&self.socket_path)
            .map_err(Error::ConnectSocket)?;

        let data = bitcode::serialize(message).map_err(Error::SerializeMessage)?;

        socket.send(&data).map_err(Error::SendMessage)?;

        Ok(())
    }
}
