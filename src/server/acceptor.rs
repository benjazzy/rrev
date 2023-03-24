mod acceptor_handle;

use std::net::SocketAddr;
use std::ops::ControlFlow;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tracing::{debug, warn};

use crate::server::connection_passer;
use crate::connection::ConnectionHdl;
use crate::parser::Parser;
use crate::server::server_handle::AcceptorsServerHandle;
pub use acceptor_handle::{AcceptorHandle, AcceptorMessage, ListenersAcceptorHandle};

/// Upgrades tcp streams to a websocket.
#[derive(Debug)]
struct Acceptor<P: Parser> {
    /// Receiver for listening for [AcceptorMessage] on.
    rx: mpsc::Receiver<AcceptorMessage>,

    /// Server handle to pass the websocket connections on to.
    server_hdl: AcceptorsServerHandle<P>,
}

impl<P: Parser> Acceptor<P> {
    /// Creates a new Acceptor.
    ///
    /// # Arguments
    /// * `rx` Receiver to listen for [AcceptorMessage] on.
    /// * `server_hdl` - Handle to pass the websockets on to.
    pub fn new(rx: mpsc::Receiver<AcceptorMessage>, server_hdl: AcceptorsServerHandle<P>) -> Self {
        Acceptor { rx, server_hdl }
    }

    /// Starts listing for new tcp streams.
    pub async fn run(mut self) {
        loop {
            let message_option = self.rx.recv().await;
            if let Some(message) = message_option {
                if self.handle_message(message).await.is_break() {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Handles an incoming [AcceptorMessage].
    /// Returns [ControlFlow::Break] if the Acceptor should exit.
    async fn handle_message(&mut self, message: AcceptorMessage) -> ControlFlow<()> {
        match message {
            AcceptorMessage::Close => return ControlFlow::Break(()),
            AcceptorMessage::NewStream(stream, addr) => self.accept_stream(stream, addr).await,
        }

        ControlFlow::Continue(())
    }

    /// Turns a tcp stream into a websocket stream.
    /// If the upgrade is successful then create a connection
    /// from the stream and pass it to the server.
    ///
    /// # Arguments
    /// * `stream` - Tcp stream to accept.
    /// * `addr` - Address of the peer of `stream`.
    async fn accept_stream(&mut self, stream: TcpStream, addr: SocketAddr) {
        let maybe_tls = MaybeTlsStream::Plain(stream);
        let ws_stream = tokio_tungstenite::accept_async(maybe_tls).await;

        match ws_stream {
            Ok(ws_stream) => {
                debug!("Accepted new websocket stream from address: {addr}");
                let (tx, rx) = mpsc::channel(1);
                let connection_hdl = ConnectionHdl::<P>::new(ws_stream, tx).await;
                tokio::spawn(connection_passer::pass_messages(
                    rx,
                    self.server_hdl.clone().into(),
                    addr,
                ));
                self.server_hdl.new_connection(connection_hdl, addr).await;
            }
            Err(e) => {
                warn!("Problem accepting new websocket stream from address: {addr}. {e}");
            }
        }
    }
}
