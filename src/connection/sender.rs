use crate::error::SendError;
use crate::parser::Parser;
use crate::scheme::internal;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use std::ops::ControlFlow;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{error, warn};

use super::internal_hdl;

pub enum SenderMessage<P: Parser> {
    Message(internal::Message<P::OurRequest, P::OurReply, P::OurEvent>),
    Close,
}

/// Sender handles the sink side of a split websocket.
/// Sender listens for messages sent from a handler and then serializes them
/// and passes them onto the websocket sink.
/// If a SenderMessage::Close is sent Sender will close its sink and shutdown.
struct Sender<P: Parser> {
    connection_hdl: internal_hdl::InternalHdl<P>,
    rx: mpsc::Receiver<SenderMessage<P>>,
    ws_sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
}

#[derive(Debug)]
pub struct SenderHdl<P: Parser> {
    tx: mpsc::Sender<SenderMessage<P>>,
}

impl<P: Parser> Sender<P> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<P>,
        rx: mpsc::Receiver<SenderMessage<P>>,
        ws_sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    ) -> Self {
        Sender {
            connection_hdl: connection_handle,
            rx,
            ws_sender,
        }
    }

    pub async fn run(mut self) {
        while let Some(request) = self.rx.recv().await {
            if let ControlFlow::Break(()) = self.handle_message(request).await {
                break;
            };
        }
    }

    async fn handle_message(&mut self, message: SenderMessage<P>) -> ControlFlow<()> {
        match message {
            SenderMessage::Message(m) => {
                self.send(m).await;
            }
            SenderMessage::Close => {
                let _ = self.ws_sender.close().await;
                return ControlFlow::Break(());
            }
        };

        ControlFlow::Continue(())
    }

    async fn send(&mut self, request: internal::Message<P::OurRequest, P::OurReply, P::OurEvent>) {
        let message_str = match serde_json::to_string(&request) {
            Ok(m) => m,
            Err(e) => {
                error!("Error serializing request! {e}");
                return;
            }
        };

        if let Err(e) = self.ws_sender.send(Message::Text(message_str)).await {
            use tokio_tungstenite::tungstenite::error::Error;
            match e {
                Error::ConnectionClosed => {
                    warn!("Could not send websocket message because connection is already closed.");
                    self.connection_hdl.close().await
                }
                Error::Protocol(_) | Error::Utf8 => {
                    error!("Unrecoverable websocket error. Closing.");
                    self.connection_hdl.close().await;
                }
                _ => {}
            }
            error!("Problem sending websocket message. {e}");
        }
    }
}

impl<P: Parser> SenderHdl<P> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<P>,
        ws_sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let sender = Sender::new(connection_handle, rx, ws_sender);

        tokio::spawn(sender.run());

        Self { tx }
    }

    pub async fn close(&self) -> Result<(), SendError> {
        self.tx
            .send(SenderMessage::Close)
            .await
            .map_err(|_| SendError)
    }

    pub async fn send(
        &self,
        message: internal::Message<P::OurRequest, P::OurReply, P::OurEvent>,
    ) -> Result<(), SendError> {
        self.tx
            .send(SenderMessage::Message(message))
            .await
            .map_err(|_| SendError)
    }
}

impl<P: Parser> Clone for SenderHdl<P> {
    fn clone(&self) -> Self {
        SenderHdl {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::internal_hdl;
    use crate::connection::internal_hdl::InternalHdl;
    use crate::connection::sender::SenderHdl;
    use crate::parser::StringParser;
    use crate::scheme::internal;
    use crate::scheme::internal::Request;
    use futures_util::StreamExt;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::{connect_async, MaybeTlsStream};

    async fn client(addr: String, tx: mpsc::Sender<String>) {
        let url = url::Url::parse(format!("ws://{addr}").as_str()).expect("Error parsing url.");

        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Error connecting to the server.");

        let (_, mut read) = ws_stream.split();
        while let Some(message) = read.next().await {
            match message {
                Ok(m) => {
                    tx.send(m.to_string()).await;
                }
                Err(_) => break,
            }
        }
    }

    async fn socket() -> (TcpListener, String) {
        let socket = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Error binding on socket address");
        let addr = socket
            .local_addr()
            .expect("Error getting socket listen address");

        (socket, addr.to_string())
    }

    async fn server(socket: TcpListener) -> SenderHdl<StringParser> {
        let (internal_tx, _internal_rx) = mpsc::channel(1);

        let internal_hdl: InternalHdl<StringParser> = internal_hdl::InternalHdl::new(internal_tx);

        let (stream, _) = socket.accept().await.expect("Error accepting connection.");

        let maybe_tls = MaybeTlsStream::Plain(stream);

        let ws_stream = tokio_tungstenite::accept_async(maybe_tls)
            .await
            .expect("Error accepting websocket stream.");

        let (write, _) = ws_stream.split();

        SenderHdl::new(internal_hdl, write)
    }

    #[tokio::test]
    async fn check_sender() {
        let message = "test";
        let request = internal::Message::Request(Request {
            id: 0,
            data: message.to_string(),
        });
        let (client_tx, mut client_rx) = mpsc::channel(1);

        let (socket, addr) = socket().await;
        tokio::spawn(client(addr, client_tx));
        let sender_hdl = server(socket).await;

        sender_hdl.send(request.clone()).await;

        let client_message = tokio::time::timeout(Duration::from_millis(100), client_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message");

        assert_eq!(
            client_message.as_str(),
            serde_json::to_string(&request).unwrap()
        );
    }
}
