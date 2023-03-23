use crate::error::SendError;
use crate::server::{acceptor::ListenersAcceptorHandle, error::ListenAddrError};
use std::net::SocketAddr;
use tokio::io;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use super::Listener;

/// Messages that can be sent from a ListenerHandle to a Listener.
/// If the message requires a reply then use a oneshot.
#[derive(Debug)]
pub enum ListenerMessage {
    /// Close the listener.
    Close,

    /// Get the address that the listener is listening on.
    GetAddress(oneshot::Sender<io::Result<SocketAddr>>),
}

/// Used to access a Listener.
/// Listener listens for incoming tcp connections.
///
/// # Examples
///
// ```
// # tokio_test::block_on(async {
// use websocket::server::ListenerHandle;
//
// let (tx, _rx) = tokio::sync::mpsc::channel(1);
// let ip = "127.0.0.1";
// let handle = ListenerHandle::new(tx, format!("{ip}:0").to_string()).await
//     .expect("Problem binding to address");
//
// let address = handle.get_addr().await.expect("Problem getting address.");
// assert_eq!(address.ip().to_string(), ip.to_string());
//
// # })
// ```
#[derive(Debug, Clone)]
pub struct ListenerHandle {
    tx: mpsc::Sender<ListenerMessage>,
}

impl ListenerHandle {
    /// Creates a new Listener and returns the associated ListenerHandle.
    /// If there is a problem binding to the address then this
    /// function will return an error.
    ///
    /// # Arguments
    /// * `addr` - Ip and Port to listen on.
    pub async fn new(acceptor_hdl: ListenersAcceptorHandle, addr: String) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel(1);

        let listener = Listener::new(rx, acceptor_hdl, addr).await?;
        tokio::spawn(listener.run());

        Ok(ListenerHandle { tx })
    }

    pub async fn close(&self) -> Result<(), SendError> {
        self.tx
            .send(ListenerMessage::Close)
            .await
            .map_err(|_| SendError)
    }

    /// Gets the address of the running Listener.
    /// If there is a problem sending the request to
    /// the Listener then return [Error::SendError].
    /// If there is a problem getting the reply back
    /// from the Listener then return [Error::RecvError].
    /// If the listener had a problem getting the listen
    /// address then return [Error::Io] with the io error attached.
    pub async fn get_addr(&self) -> Result<SocketAddr, ListenAddrError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ListenerMessage::GetAddress(tx))
            .await
            .map_err(|_| ListenAddrError::SendError)?;
        let addr = rx.await.map_err(|_| ListenAddrError::RecvError)?;

        return addr.map_err(|e| ListenAddrError::Io(e));
    }
}
