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
pub enum Error {}

pub fn new(socket_path: PathBuf) -> Client {
    Client { socket_path }
}

impl Client {
    pub fn send(&self, message: Message) -> Result<(), Error> {
        let socket = UnixDatagram::unbound().unwrap();
        socket.connect(&self.socket_path).unwrap();
        socket.send(&bincode::serialize(&message).unwrap()).unwrap();

        Ok(())
    }
}
