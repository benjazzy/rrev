use crate::parser::Parser;
use crate::server::acceptor::Acceptor;
use crate::server::server_handle::AcceptorsServerHandle;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::error::SendError;

#[derive(Debug)]
pub enum AcceptorMessage {
    Close,
    NewStream(TcpStream, SocketAddr),
}

#[derive(Debug, Clone)]
pub struct AcceptorHandle {
    tx: mpsc::Sender<AcceptorMessage>,
}

#[derive(Debug, Clone)]
pub struct ListenersAcceptorHandle {
    tx: mpsc::Sender<AcceptorMessage>,
}

impl AcceptorHandle {
    pub fn new<P: Parser>(server_hdl: AcceptorsServerHandle<P>) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let acceptor = Acceptor::new(rx, server_hdl);
        tokio::spawn(acceptor.run());

        AcceptorHandle { tx }
    }
    
    pub async fn close(&self) -> Result<(), SendError> {
        self.tx.send(AcceptorMessage::Close).await.map_err(|_| SendError)
    }
}

impl ListenersAcceptorHandle {
    pub fn new(tx: mpsc::Sender<AcceptorMessage>) -> Self {
        ListenersAcceptorHandle { tx }
    }

    pub async fn new_stream(&self, stream: TcpStream, addr: SocketAddr) {
        self.tx.send(AcceptorMessage::NewStream(stream, addr)).await;
    }
}

#[cfg(test)]
impl AcceptorHandle {
    pub fn from_tx(tx: mpsc::Sender<AcceptorMessage>) -> Self {
        AcceptorHandle { tx }
    }
}

impl From<AcceptorHandle> for ListenersAcceptorHandle {
    fn from(value: AcceptorHandle) -> Self {
        ListenersAcceptorHandle { tx: value.tx }
    }
}
