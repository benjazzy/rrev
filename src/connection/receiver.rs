use crate::parser::Parser;
use futures_util::stream::SplitStream;
use futures_util::StreamExt;
use serde::Deserialize;
use std::ops::ControlFlow;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error};
use crate::error::SendError;

use super::internal_hdl;

enum ReceiverMessage {
    Close,
}

/// Receiver handles the stream side of a split websocket.
/// It listens for messages on the websocket stream deserializes it
/// and calls Connection's internal handle to pass the message up the chain.
/// If the websocket message is a close message or unhandleable error Receiver
/// calls close on Connection's internal handle to close the connection.
/// Receiver also waits for a message from Connection and handles the message.
struct Receiver<P: Parser> {
    connection_handle: internal_hdl::InternalHdl<P>,
    rx: mpsc::Receiver<ReceiverMessage>,
    ws_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

pub struct ReceiverHdl {
    tx: mpsc::Sender<ReceiverMessage>,
}

impl<P: Parser> Receiver<P> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<P>,
        rx: mpsc::Receiver<ReceiverMessage>,
        ws_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Self {
        Receiver {
            connection_handle,
            rx,
            ws_receiver,
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(message) = self.rx.recv() => {
                    if let ControlFlow::Break(()) = self.handle_connection_message(message) {
                        break;
                    };
                },
                message = self.ws_receiver.next() => {
                    if let ControlFlow::Break(()) = self.handle_ws_message(message).await {
                        break;
                    };
                }

            }
        }
        debug!("Receiver exiting.");
    }

    fn handle_connection_message(&self, message: ReceiverMessage) -> ControlFlow<()> {
        match message {
            ReceiverMessage::Close => {
                return ControlFlow::Break(());
            }
        };
    }

    async fn handle_ws_message(
        &self,
        try_message: Option<Result<Message, tungstenite::Error>>,
    ) -> ControlFlow<()> {
        let message = if let Some(result) = try_message {
            match result {
                Ok(m) => m,
                Err(e) => {
                    use tungstenite::Error;
                    match e {
                        Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => {
                            debug!("Receiver connection closed.");
                            self.connection_handle.close().await;
                            return ControlFlow::Break(());
                        }
                        _ => {
                            error!("Receiver websocket error! {e}");
                            return ControlFlow::Continue(());
                        }
                    };
                }
            }
        } else {
            debug!("Receiver got an empty message. Closing.");
            self.connection_handle.close().await;
            return ControlFlow::Break(());
        };

        let message_str = if let Ok(m) = message.to_text() {
            m
        } else {
            error!("Message is not text");
            return ControlFlow::Continue(());
        };

        let try_message = serde_json::from_str(message_str);
        match try_message {
            Ok(m) => self.connection_handle.new_message(m).await,
            Err(e) => error!("Problem deserializing message! {e}"),
        };

        ControlFlow::Continue(())
    }
}

impl ReceiverHdl {
    pub fn new<P: Parser>(
        connection_handle: internal_hdl::InternalHdl<P>,
        ws_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let receiver = Receiver::new(connection_handle, rx, ws_receiver);

        tokio::spawn(receiver.run());

        ReceiverHdl { tx }
    }

    pub async fn close(&self) -> Result<(), SendError> {
        self.tx.send(ReceiverMessage::Close).await.map_err(|_| SendError)
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::internal_hdl;
    use crate::connection::internal_hdl::InternalMessage;
    use crate::connection::receiver::ReceiverHdl;
    use crate::parser::StringParser;
    use crate::scheme::internal;
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{connect_async, MaybeTlsStream};

    async fn client(addr: String, mut rx: mpsc::Receiver<Option<String>>) {
        let url = url::Url::parse(format!("ws://{addr}").as_str()).expect("Error parsing url.");

        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Error connecting to the server.");

        let (mut write, _) = ws_stream.split();

        while let Some(Some(message)) = rx.recv().await {
            write
                .send(Message::Text(message))
                .await
                .expect("Problem sending message.");
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

    async fn server(
        socket: TcpListener,
    ) -> (ReceiverHdl, mpsc::Receiver<InternalMessage<StringParser>>) {
        let (internal_tx, internal_rx) = mpsc::channel(1);

        let internal_hdl = internal_hdl::InternalHdl::new(internal_tx);

        let (stream, _) = socket.accept().await.expect("Error accepting connection.");

        let maybe_tls = MaybeTlsStream::Plain(stream);

        let ws_stream = tokio_tungstenite::accept_async(maybe_tls)
            .await
            .expect("Error accepting websocket stream.");

        let (_, read) = ws_stream.split();

        let receiver_hdl = ReceiverHdl::new(internal_hdl, read);

        (receiver_hdl, internal_rx)
    }

    async fn try_message(
        message: internal::Message<String, String, String>,
        client_tx: mpsc::Sender<Option<String>>,
        connection_rx: &mut mpsc::Receiver<InternalMessage<StringParser>>,
    ) {
        client_tx
            .send(Some(serde_json::to_string(&message).unwrap()))
            .await
            .expect("Problem sending message.");

        let receiver_message =
            tokio::time::timeout(Duration::from_millis(100), connection_rx.recv())
                .await
                .expect("Timeout unwrapping receiver message.")
                .expect("Problem unwrapping receiver message.");

        assert_eq!(receiver_message, InternalMessage::NewMessage(message));
    }

    #[tokio::test]
    async fn check_receiver() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request {
            id: 0,
            data: message.to_string(),
        });
        let reply = internal::Message::<String, String, String>::Reply(internal::Reply {
            id: 1,
            data: message.to_string(),
        });
        let event = internal::Message::<String, String, String>::Event(message.to_string());
        let (client_tx, mut client_rx) = mpsc::channel(1);

        let (socket, addr) = socket().await;
        tokio::spawn(client(addr, client_rx));
        let (_receiver_hdl, mut connection_rx) = server(socket).await;

        try_message(request, client_tx.clone(), &mut connection_rx).await;
        try_message(reply, client_tx.clone(), &mut connection_rx).await;
        try_message(event, client_tx.clone(), &mut connection_rx).await;
    }
}
