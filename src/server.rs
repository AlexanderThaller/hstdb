use std::os::unix::net::UnixDatagram;
use thiserror::Error;

const BUFFER_SIZE: usize = 65_527;

pub struct Server {}

#[derive(Error, Debug)]
pub enum Error {}

impl Server {
    pub fn start() -> Result<(), Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let socket_path = xdg_dirs.place_runtime_file("socket").unwrap();

        dbg!(&socket_path);

        let socket = UnixDatagram::bind(&socket_path).unwrap();

        loop {
            if let Err(err) = Self::receive(&socket) {
                eprintln!("{}", err)
            }
        }

        // std::fs::remove_file(&socket_path).unwrap();
    }

    fn receive(socket: &UnixDatagram) -> Result<(), Error> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let (written, _) = socket.recv_from(&mut buffer).unwrap();

        dbg!(String::from_utf8_lossy(&buffer[0..written]));

        Ok(())
    }
}
