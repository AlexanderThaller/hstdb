use crate::message::Message;
use std::os::unix::net::UnixDatagram;
use thiserror::Error;

#[derive(Debug)]
pub struct Client {}

#[derive(Error, Debug)]
pub enum Error {}

pub fn new() -> Client {
    Client {}
}

impl Client {
    pub fn send(&self, message: Message) -> Result<(), Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let socket_path = xdg_dirs.find_runtime_file("socket").unwrap();

        let socket = UnixDatagram::unbound().unwrap();
        socket.connect(socket_path).unwrap();
        socket.send(&bincode::serialize(&message).unwrap()).unwrap();

        Ok(())
    }
}
