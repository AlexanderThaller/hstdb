use super::Builder;
use crate::{
    client,
    message::Message,
    store::Store,
};
use crossbeam_utils::sync::WaitGroup;
use flume::{
    Receiver,
    Sender,
};
use log::warn;
use std::{
    os::unix::net::UnixDatagram,
    path::PathBuf,
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
    thread,
};
use thiserror::Error;

const BUFFER_SIZE: usize = 65_527;

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not receive message from socket: {0}")]
    ReceiveFromSocket(std::io::Error),

    #[error("can not send received data to processing: {0}")]
    SendBuffer(flume::SendError<Vec<u8>>),

    #[error("can not deserialize message: {0}")]
    DeserializeMessage(bincode::Error),

    #[error("can not receive data from channel: {0}")]
    ReceiveData(flume::RecvError),

    #[error("can not remove socket: {0}")]
    RemoveSocket(std::io::Error),

    #[error("can not setup ctrlc handler: {0}")]
    SetupCtrlHandler(ctrlc::Error),
}

pub struct Server {
    pub(super) entries: sled::Db,
    pub(super) socket: UnixDatagram,
    pub(super) socket_path: PathBuf,
    pub(super) store: Store,
    pub(super) stopping: Arc<AtomicBool>,
    pub(super) wait_group: WaitGroup,
}

pub fn builder(cache_dir: PathBuf, data_dir: PathBuf, socket: PathBuf) -> Builder {
    Builder {
        cache_dir,
        data_dir,
        socket,
    }
}

impl Server {
    pub fn run(self) -> Result<(), Error> {
        let data_sender =
            Self::start_processor(Arc::clone(&self.stopping), self.wait_group.clone())?;

        Self::start_receiver(
            Arc::clone(&self.stopping),
            self.wait_group.clone(),
            self.socket,
            data_sender,
        )?;

        Self::ctrl_c_watcher(self.stopping, self.socket_path.clone())?;

        self.wait_group.wait();

        std::fs::remove_file(&self.socket_path).map_err(Error::RemoveSocket)?;

        Ok(())
    }

    fn ctrl_c_watcher(stopping: Arc<AtomicBool>, socket_path: PathBuf) -> Result<(), Error> {
        ctrlc::set_handler(move || {
            stopping.store(true, Ordering::SeqCst);

            let client = client::new(socket_path.clone());
            if let Err(err) = client.send(&Message::Stop) {
                warn!("{}", err);
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
    ) -> Result<(), Error> {
        thread::spawn(move || {
            loop {
                if dbg!(stopping.load(Ordering::SeqCst)) {
                    dbg!("break loop");
                    break;
                }

                if let Err(err) = Self::receive(&socket, &data_sender) {
                    warn!("{}", err)
                }
            }

            drop(wait_group)
        });

        Ok(())
    }

    fn receive(socket: &UnixDatagram, data_sender: &Sender<Vec<u8>>) -> Result<(), Error> {
        dbg!("receive");

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
    ) -> Result<Sender<Vec<u8>>, Error> {
        let (data_sender, data_receiver) = flume::unbounded();

        thread::spawn(move || {
            loop {
                if dbg!(stopping.load(Ordering::SeqCst)) {
                    break;
                }

                if let Err(err) = Self::process(&stopping, &data_receiver) {
                    warn!("{}", err)
                }
            }

            drop(wait_group)
        });

        Ok(data_sender)
    }

    fn process(stopping: &Arc<AtomicBool>, data_receiver: &Receiver<Vec<u8>>) -> Result<(), Error> {
        let data = data_receiver.recv().map_err(Error::ReceiveData)?;
        let message = bincode::deserialize(&data).map_err(Error::DeserializeMessage)?;

        dbg!(&message);

        match message {
            Message::Stop => stopping.store(true, Ordering::SeqCst),
            _ => todo!(),
        }

        Ok(())
    }
}
