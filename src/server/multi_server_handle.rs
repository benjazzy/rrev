use crate::connection::ConnectionHdl;
use crate::error::SendError;
use crate::parser::Parser;
use crate::server::server_handle::ServerMessage;
use crate::server::server_trait::Server;
use crate::server::ServerEvent;
use std::net::SocketAddr;
use tokio::sync::mpsc;

pub struct MultiServerHandle<P: Parser> {
    tx: mpsc::Sender<ServerMessage<P>>,
}

pub struct AcceptorsMultiServerHandle<P: Parser> {
    tx: mpsc::Sender<ServerMessage<P>>,
}

impl<P: Parser> MultiServerHandle<P> {
    pub async fn new<S: Server<P>>(event_tx: mpsc::Sender<ServerEvent<P>>, server: S) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let server = S::new(rx, event_tx);

        tokio::spawn(server.run());

        MultiServerHandle { tx }
    }
}

impl<P: Parser> AcceptorsMultiServerHandle<P> {
    /// Creates a new AcceptorsMultiServerHandle from a Sender.
    ///
    /// # Arguments
    /// * `tx` - Sender to send messages to the server.
    pub fn new(tx: mpsc::Sender<ServerMessage<P>>) -> Self {
        AcceptorsMultiServerHandle { tx }
    }

    /// Called to send a new accepted connection to the server.
    ///
    /// # Arguments
    /// * `connection_hdl` - The handle of the new connection.
    /// * `addr` - The address of the new connection.
    pub async fn new_connection(
        &self,
        connection_hdl: ConnectionHdl<P>,
        addr: SocketAddr,
    ) -> Result<(), SendError> {
        self.tx
            .send(ServerMessage::NewConnection(connection_hdl, addr))
            .await
            .map_err(|_| SendError)
    }
}

impl<P: Parser> From<MultiServerHandle<P>> for AcceptorsMultiServerHandle<P> {
    fn from(value: MultiServerHandle<P>) -> Self {
        AcceptorsMultiServerHandle::new(value.tx)
    }
}
