use std::{
    ffi::OsString,
    os::unix::net::UnixDatagram,
    path::{
        Path,
        PathBuf,
    },
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    message::{
        Message,
        Response,
    },
    version::VERSION,
};

const BUFFER_SIZE: usize = 16_384;
const SERVER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Sends serialized [`Message`] values to the local `hstdb` Unix socket.
#[derive(Debug)]
pub struct Client {
    socket_path: PathBuf,
}

/// Errors returned while sending a message to the server.
#[derive(Error, Debug)]
pub enum Error {
    /// The configured socket path has no parent directory.
    #[error("socket path {0} has no parent directory")]
    NoSocketPathParent(PathBuf),

    /// The server did not publish its version metadata next to the socket.
    #[error(
        "server version file {0} is missing; restart the hstdb server after upgrading the client"
    )]
    MissingServerVersionFile(PathBuf),

    /// Reading the server version file failed.
    #[error("can not read server version file {0}: {1}")]
    ReadServerVersion(PathBuf, std::io::Error),

    /// The client and server versions do not match.
    #[error(
        "client/server version mismatch: client {client_version}, server {server_version}; \
         restart or upgrade the hstdb server"
    )]
    ServerVersionMismatch {
        /// Client version string.
        client_version: &'static str,
        /// Server version string.
        server_version: String,
    },

    /// Binding the temporary reply socket failed.
    #[error("can not bind reply socket at path {0}: {1}")]
    BindReplySocket(PathBuf, std::io::Error),

    /// Configuring the server response timeout failed.
    #[error("can not configure reply socket timeout: {0}")]
    SetReadTimeout(std::io::Error),

    /// Serializing the outgoing message failed.
    #[error("can not serialize message: {0}")]
    SerializeMessage(bitcode::Error),

    /// Writing the serialized message to the socket failed.
    #[error("can not send message to socket: {0}")]
    SendMessage(std::io::Error),

    /// Receiving the server response failed.
    #[error("can not receive server response: {0}")]
    ReceiveResponse(std::io::Error),

    /// Deserializing the server response failed.
    #[error("can not deserialize server response: {0}")]
    DeserializeResponse(bitcode::Error),

    /// The server rejected the request.
    #[error("{0}")]
    ServerError(String),
}

/// Creates a client that targets the given Unix domain socket path.
#[must_use]
pub const fn new(socket_path: PathBuf) -> Client {
    Client { socket_path }
}

impl Client {
    /// Serializes and sends a message to the configured server socket.
    pub fn send(&self, message: &Message) -> Result<(), Error> {
        self.ensure_server_version()?;

        let reply_socket = ReplySocket::new(&self.socket_path)?;

        let data = bitcode::serialize(message).map_err(Error::SerializeMessage)?;

        reply_socket
            .socket
            .send_to(&data, &self.socket_path)
            .map_err(Error::SendMessage)?;

        let mut buffer = [0_u8; BUFFER_SIZE];
        let written = reply_socket
            .socket
            .recv(&mut buffer)
            .map_err(Error::ReceiveResponse)?;

        match bitcode::deserialize(&buffer[..written]).map_err(Error::DeserializeResponse)? {
            Response::Ok => Ok(()),
            Response::Error(message) => Err(Error::ServerError(message)),
        }
    }

    fn ensure_server_version(&self) -> Result<(), Error> {
        let version_path = socket_version_path(&self.socket_path);
        let server_version =
            std::fs::read_to_string(&version_path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => {
                    Error::MissingServerVersionFile(version_path.clone())
                }
                _ => Error::ReadServerVersion(version_path.clone(), err),
            })?;
        let server_version = server_version.trim().to_string();

        if server_version == VERSION {
            Ok(())
        } else {
            Err(Error::ServerVersionMismatch {
                client_version: VERSION,
                server_version,
            })
        }
    }
}

#[derive(Debug)]
struct ReplySocket {
    socket: UnixDatagram,
    path: PathBuf,
}

impl ReplySocket {
    fn new(server_socket_path: &Path) -> Result<Self, Error> {
        let parent = server_socket_path
            .parent()
            .ok_or_else(|| Error::NoSocketPathParent(server_socket_path.to_path_buf()))?;
        let path = parent.join(format!(".hstdb-client-{}.sock", Uuid::new_v4()));
        let socket =
            UnixDatagram::bind(&path).map_err(|err| Error::BindReplySocket(path.clone(), err))?;
        socket
            .set_read_timeout(Some(SERVER_RESPONSE_TIMEOUT))
            .map_err(Error::SetReadTimeout)?;

        Ok(Self { socket, path })
    }
}

impl Drop for ReplySocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[must_use]
pub(crate) fn socket_version_path(socket_path: &Path) -> PathBuf {
    let mut path = OsString::from(socket_path.as_os_str());
    path.push(".version");
    PathBuf::from(path)
}
