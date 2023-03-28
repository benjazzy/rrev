use crate::error::SendError;
use crate::parser::Parser;
use crate::server::acceptor::{AcceptorMessage, ListenersAcceptorHandle};
use crate::server::error::ListenAddrError;
use crate::server::multi_acceptor::MultiAcceptor;
use crate::server::server_handle::AcceptorsServerHandle;
use crate::server::ListenerHandle;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use tokio::io;
use tokio::sync::mpsc;

pub struct MultiAcceptorHandle {
    tx: mpsc::Sender<AcceptorMessage>,
    listener_hdl: ListenerHandle,
}

impl MultiAcceptorHandle {
    pub async fn new<P: Parser>(
        listen_addr: String,
        server_hdls: HashMap<String, AcceptorsServerHandle<P>>,
    ) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel(1);
        let listener_hdl =
            ListenerHandle::new(ListenersAcceptorHandle::new(tx.clone()), listen_addr).await?;
        let acceptor: MultiAcceptor<P> = MultiAcceptor::new(rx, server_hdls);

        tokio::spawn(acceptor.run());

        Ok(MultiAcceptorHandle { tx, listener_hdl })
    }

    pub async fn get_listen_address(&self) -> Result<SocketAddr, ListenAddrError> {
        self.listener_hdl.get_addr().await
    }

    pub async fn close(&self) -> Result<(), SendError> {
        let _ = self.listener_hdl.close().await;

        self.tx
            .send(AcceptorMessage::Close)
            .await
            .map_err(|_| SendError)
    }
}

// impl From<MultiAcceptorHandle> for ListenersAcceptorHandle {
//     fn from(value: MultiAcceptorHandle) -> Self {
//         ListenersAcceptorHandle::new(value.tx)
//     }
// }

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn check_one() {}
}
