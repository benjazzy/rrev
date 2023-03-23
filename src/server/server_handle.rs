use crate::connection::{ConnectionEvent, ConnectionHdl, SenderHdl};
use crate::parser::Parser;
use crate::error::{RequestError, SendError, TimeoutError};
use crate::scheme::RequestHandle;
use crate::server::server_event::ServerEvent;
use crate::server::server_handle::ServerMessage::SendRequest;
use crate::server::Server;
use crate::server::error::{ ClientsError, ListenAddrError };
use std::net::SocketAddr;
use std::time::Duration;
use futures_util::TryFutureExt;
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
    GetListenAddress(oneshot::Sender<Result<SocketAddr, ListenAddrError>>),

    /// Requests the list of connected clients.
    /// The oneshot allows the server to send back the clients.
    GetClients(oneshot::Sender<Result<Vec<SocketAddr>, ClientsError>>),

    NewConnection(ConnectionHdl<P>, SocketAddr),

    ConnectionEvent {
        from: SocketAddr,
        event: ConnectionEvent<P>,
    },

    /// Sends a request to a client.
    /// # Arguments
    /// * `to` - Client url to send the request to.
    /// * `request` - Request to send.
    SendRequest {
        to: SocketAddr,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
        request: P::OurRequest,
    },

    /// Sends an event to a client.
    /// # Arguments
    /// * `to` - Client url to send the event to.
    /// * `event` - Event to send.
    SendEvent {
        to: SocketAddr,
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

/// Used to manage the server externally.
#[derive(Debug, Clone)]
pub struct PassersServerHandle<P: Parser> {
    /// Sender to send ServerMessages to the server.
    tx: mpsc::Sender<ServerMessage<P>>,
}

impl<P: Parser> ServerHandle<P> {
    /// Creates a new ServerHandler and Server.
    /// Returns the handler and starts the Server.
    ///
    /// # Arguments
    /// * `listen_addr` - Addresses to listen on
    pub async fn new(
        listen_addr: String,
        server_event_tx: mpsc::Sender<ServerEvent<P>>,
    ) -> tokio::io::Result<Self> {
        let (hdl_tx, hdl_rx) = mpsc::channel(1);

        let server = Server::<P>::new(hdl_rx, hdl_tx.clone(), server_event_tx, listen_addr).await?;
        tokio::spawn(server.run());

        Ok(ServerHandle { tx: hdl_tx })
    }

    /// Closes the server.
    pub async fn close(&self) -> Result<(), SendError> {
        self.tx.send(ServerMessage::Close).await.map_err(|_| SendError)
    }

    pub async fn get_listen_address(&self) -> Result<SocketAddr, ListenAddrError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(ServerMessage::GetListenAddress(tx)).await.map_err(|_| ListenAddrError::SendError)?;
        let result = rx.await;

        match result {
            Ok(r) => r,
            Err(_) => Err(ListenAddrError::RecvError),
        }
    }

    pub async fn get_clients(&self) -> Result<Vec<SocketAddr>, ClientsError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ServerMessage::GetClients(tx))
            .await
            .map_err(|_| ClientsError::SendError)?;
        let result = rx.await;

        match result {
            Ok(r) => r,
            Err(_) => Err(ClientsError::RecvError),
        }
    }

    pub async fn request(
        &self,
        to: SocketAddr,
        request: P::OurRequest,
    ) -> Result<P::TheirReply, RequestError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(SendRequest { to, tx, request }).await.map_err(|_| RequestError::SendError)?;

        match rx.await {
            Ok(r) => r,
            Err(_) => Err(RequestError::RecvError),
        }
    }

    pub async fn request_timeout(
        &self,
        to: SocketAddr,
        request: P::OurRequest,
        timeout: Duration,
    ) -> Result<P::TheirReply, TimeoutError> {
        match tokio::time::timeout(timeout, self.request(to, request)).await {
            Ok(result) => result.map_err(|e| TimeoutError::RequestError(e)),
            Err(_) => return Err(TimeoutError::Timeout),
        }
    }

    pub async fn event(&self, to: SocketAddr, event: P::OurEvent) -> Result<(), SendError> {
        self.tx.send(ServerMessage::SendEvent { to, event }).await.map_err(|_| SendError)
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
    pub async fn new_connection(&self, connection_hdl: ConnectionHdl<P>, addr: SocketAddr) -> Result<(), SendError> {
        self.tx
            .send(ServerMessage::NewConnection(connection_hdl, addr))
            .await
            .map_err(|_| SendError)
    }
}

impl<P: Parser> PassersServerHandle<P> {
    pub async fn new_connection_event(&self, event: ConnectionEvent<P>, from: SocketAddr) -> Result<(), SendError> {
        self.tx
            .send(ServerMessage::ConnectionEvent { from, event })
            .await
            .map_err(|_| SendError)
    }
}

impl<P: Parser> From<AcceptorsServerHandle<P>> for PassersServerHandle<P> {
    fn from(value: AcceptorsServerHandle<P>) -> Self {
        PassersServerHandle { tx: value.tx }
    }
}
