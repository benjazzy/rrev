use std::net::SocketAddr;
use tokio::io;
use tokio::sync::{mpsc, oneshot };

use super::Listener;

#[derive(Debug)]
pub enum ListenerMessage {
    Close,
    GetAddress(oneshot::Sender<io::Result<SocketAddr>>),
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    RecvError,
    SendError,
}

#[derive(Debug, Clone)]
pub struct ListenerHandle {
    tx: mpsc::Sender<ListenerMessage>,
}

impl ListenerHandle {
    pub async fn new(addr: String) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel(1);

        let listener = Listener::new(rx, addr).await?;
        tokio::spawn(listener.run());

        Ok(ListenerHandle { tx })
    }

    pub async fn get_addr(&self) -> Result<SocketAddr, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(ListenerMessage::GetAddress(tx)).await.map_err(|_| Error::SendError)?;
        let addr = rx.await.map_err(|_| Error::RecvError)?;

        return addr.map_err(|e| Error::Io(e));
    }
}