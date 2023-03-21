use crate::server::ListenerHandle;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, warn};

mod acceptor_handle;

use crate::connection::ConnectionHdl;
use crate::parser::Parser;
use crate::server::server_handle::AcceptorsServerHandle;
pub use acceptor_handle::{AcceptorHandle, AcceptorMessage, ListenersAcceptorHandle};

#[derive(Debug)]
struct Acceptor<P: Parser> {
    rx: mpsc::Receiver<AcceptorMessage>,
    server_hdl: AcceptorsServerHandle<P>,
}

impl<P: Parser> Acceptor<P> {
    pub fn new(rx: mpsc::Receiver<AcceptorMessage>, server_hdl: AcceptorsServerHandle<P>) -> Self {
        Acceptor { rx, server_hdl }
    }

    pub async fn run(mut self) {
        loop {
            let message_option = self.rx.recv().await;
            if let Some(message) = message_option {
                if let AcceptorMessage::Close = message {
                    break;
                }
                self.handle_message(message).await;
            } else {
                break;
            }
        }
    }

    async fn handle_message(&mut self, message: AcceptorMessage) {
        match message {
            AcceptorMessage::Close => {}
            AcceptorMessage::NewStream(stream, addr) => self.accept_stream(stream, addr).await,
        }
    }

    async fn accept_stream(&mut self, stream: TcpStream, addr: SocketAddr) {
        let maybe_tls = MaybeTlsStream::Plain(stream);
        let ws_stream = tokio_tungstenite::accept_async(maybe_tls).await;

        match ws_stream {
            Ok(ws_stream) => {
                debug!("Accepted new websocket stream from address: {addr}");
                let connection_hdl = ConnectionHdl::<P>::new(ws_stream).await;
                self.server_hdl.new_connection(connection_hdl, addr).await;
            }
            Err(e) => {
                warn!("Problem accepting new websocket stream from address: {addr}. {e}");
            }
        }
    }
}
