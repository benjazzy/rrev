use crate::connection::ConnectionHdl;
use crate::parser::Parser;
use crate::server::{Error, Server};
use std::net::SocketAddr;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};

/// Messages that can be passed to the server.
/// They can by passed in by any server handler.
#[derive(Debug)]
pub enum ServerMessage<P: Parser> {
    /// Tells the server to close.
    Close,

    /// Requests the listening address of the server.
    /// The oneshot allows the server to send the address.
    GetListenAddress(oneshot::Sender<Result<SocketAddr, Error>>),

    /// Requests the list of connected clients.
    /// The oneshot allows the server to send back the clients.
    GetClients(oneshot::Sender<Result<Vec<SocketAddr>, Error>>),

    NewConnection(ConnectionHdl<P>, SocketAddr),

    /// Sends a request to a client.
    /// # Arguments
    /// * `to` - Client url to send the request to.
    /// * `request` - Request to send.
    SendRequest {
        to: String,
        request: P::OurRequest,
    },

    /// Sends an event to a client.
    /// # Arguments
    /// * `to` - Client url to send the event to.
    /// * `event` - Event to send.
    SendEvent {
        to: String,
        event: P::OurEvent,
    },
}

/// Used to manage the server externally.
#[derive(Debug, Clone)]
pub struct ServerHandle<P: Parser> {
    /// Sender to send ServerMessages to the server.
    tx: mpsc::Sender<ServerMessage<P>>,
}

/// Used to manage the server externally.
#[derive(Debug, Clone)]
pub struct AcceptorsServerHandle<P: Parser> {
    /// Sender to send ServerMessages to the server.
    tx: mpsc::Sender<ServerMessage<P>>,
}

impl<P: Parser> ServerHandle<P> {
    /// Creates a new ServerHandler and Server.
    /// Returns the handler and starts the Server.
    ///
    /// # Arguments
    /// * `listen_addr` - Addresses to listen on
    pub async fn new(listen_addr: String) -> tokio::io::Result<Self> {
        let (tx, rx) = mpsc::channel(1);

        let server = Server::<P>::new(rx, tx.clone(), listen_addr).await?;
        tokio::spawn(server.run());

        Ok(ServerHandle { tx })
    }

    /// Closes the server.
    pub async fn close(&self) {
        self.tx.send(ServerMessage::Close);
    }

    pub async fn get_listen_address(&self) -> Result<SocketAddr, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(ServerMessage::GetListenAddress(tx)).await;
        let result = rx.await;

        match result {
            Ok(r) => r,
            Err(_) => Err(Error::RecvError),
        }
    }

    pub async fn get_clients(&self) -> Result<Vec<SocketAddr>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ServerMessage::GetClients(tx))
            .await
            .map_err(|_| Error::SendError)?;
        let result = rx.await;

        match result {
            Ok(r) => r,
            Err(_) => Err(Error::RecvError),
        }
    }
}

impl<P: Parser> AcceptorsServerHandle<P> {
    /// Creates a new AcceptorsServerHandle from a Sender.
    ///
    /// # Arguments
    /// * `tx` - Sender to send messages to the server.
    pub fn new(tx: mpsc::Sender<ServerMessage<P>>) -> Self {
        AcceptorsServerHandle { tx }
    }

    /// Called to send a new accepted connection to the server.
    ///
    /// # Arguments
    /// * `connection_hdl` - The handle of the new connection.
    pub async fn new_connection(&self, connection_hdl: ConnectionHdl<P>, addr: SocketAddr) {
        self.tx.send(ServerMessage::NewConnection(connection_hdl, addr)).await;
    }
}
