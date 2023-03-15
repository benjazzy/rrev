use std::ops::ControlFlow;
use futures_util::StreamExt;
use futures_util::stream::SplitStream;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error};

use super::internal_hdl;

enum ReceiverMessage {
    Close,
}

struct Receiver<
    Req: for<'a> Deserialize<'a>,
    Rep: for<'a> Deserialize<'a>,
    Event: for<'a> Deserialize<'a>
> {
    connection_handle: internal_hdl::InternalHdl<Req, Rep, Event>,
    rx: mpsc::Receiver<ReceiverMessage>,
    ws_receiver: SplitStream<WebSocketStream<TcpStream>>,
}

pub struct ReceiverHdl {
    tx: mpsc::Sender<ReceiverMessage>,
}

impl<
    Req: for<'a> Deserialize<'a>,
    Rep: for<'a> Deserialize<'a>,
    Event: for<'a> Deserialize<'a>
> Receiver<Req, Rep, Event> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<Req, Rep, Event>,
        rx: mpsc::Receiver<ReceiverMessage>,
        ws_receiver: SplitStream<WebSocketStream<TcpStream>>
    ) -> Self 
    {
        Receiver { connection_handle, rx, ws_receiver }
    }
    
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(message) = self.rx.recv() => {
                    if let ControlFlow::Break(()) = self.handle_connection_message(message) {
                        break;
                    }
                },
                Some(message) = self.ws_receiver.next() => {

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

    async fn handle_ws_message(&self, message: Message) {
        let message_str = if let Ok(m) = message.to_text() {
            m
        } else {
            error!("Message is not text");
            return;
        };

        let try_message = serde_json::from_str(message_str);
        match try_message {
            Ok(m) => self.connection_handle.new_message(m).await,
            Err(e) => error!("Problem deserializing message! {e}"),
        }
    }
}

impl ReceiverHdl {
    pub fn new<
        Req: for<'a> Deserialize<'a> + Send + 'static,
        Rep: for<'a> Deserialize<'a> + Send + 'static,
        Event: for<'a> Deserialize<'a> + Send + 'static
    > (
        connection_handle: internal_hdl::InternalHdl<Req, Rep, Event>,
        ws_receiver: SplitStream<WebSocketStream<TcpStream>>
    ) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let receiver = Receiver::new(connection_handle, rx, ws_receiver);

        tokio::spawn(receiver.run());

        ReceiverHdl { tx }
    }

    pub async fn close(&self) {
        let _ = self.tx.send(ReceiverMessage::Close).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use crate::scheme::internal;
    use crate::server::connection::internal_hdl;
    use crate::server::connection::internal_hdl::InternalMessage;
    use crate::server::connection::receiver::{Receiver, ReceiverHdl};

    async fn client(addr: String, mut rx: mpsc::Receiver<Option<String>>) {
        let url = url::Url::parse(format!("ws://{addr}").as_str())
            .expect("Error parsing url.");

        let (ws_stream, _) = connect_async(url).await
            .expect("Error connecting to the server.");

        let (mut write, _) = ws_stream.split();

        while let Some(Some(message)) = rx.recv().await {
            write.send(Message::Text(message)).await.expect("Problem sending message.");
        }
    }

    async fn socket() -> (TcpListener, String) {
        let socket = TcpListener::bind("127.0.0.1:0").await
            .expect("Error binding on socket address");
        let addr = socket.local_addr()
            .expect("Error getting socket listen address");

        (socket, addr.to_string())
    }

    async fn server(socket: TcpListener) -> (ReceiverHdl, mpsc::Receiver<InternalMessage<String, String, String>>) {
        let (internal_tx, internal_rx)
            = mpsc::channel(1);

        let internal_hdl = internal_hdl::InternalHdl::new(internal_tx);

        let (stream, _) = socket.accept().await
            .expect("Error accepting connection.");

        let ws_stream =
            tokio_tungstenite::accept_async(stream).await
                .expect("Error accepting websocket stream.");

        let (_, read) = ws_stream.split();

        let receiver_hdl = ReceiverHdl::new(internal_hdl, read);

        (receiver_hdl, internal_rx)
    }

    async fn try_message(
        message: internal::Message<String, String, String>,
        client_tx: mpsc::Sender<Option<String>>,
        connection_rx: &mut mpsc::Receiver<InternalMessage<String, String, String>>,
    ) {
        client_tx.send(Some(serde_json::to_string(&message).unwrap())).await
            .expect("Problem sending message.");

        let receiver_message = tokio::time::timeout(
            Duration::from_millis(100), connection_rx.recv()).await
            .expect("Timeout unwrapping receiver message.")
            .expect("Problem unwrapping receiver message.");

        assert_eq!(receiver_message, internal_hdl::InternalMessage::NewMessage(message));
    }

    #[tokio::test]
    async fn check_receiver() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request { id: 0, data: message.to_string() });
        let reply = internal::Message::<String, String, String>::Reply(internal::Reply { id: 1, data: message.to_string() });
        let event = internal::Message::<String, String, String>::Event(message.to_string());
        let (client_tx, mut client_rx) = mpsc::channel(1);

        let (socket, addr) = socket().await;
        tokio::spawn(client(addr, client_rx));
        let (receiver_hdl, mut connection_rx) = server(socket).await;

        try_message(request, client_tx.clone(), &mut connection_rx);

        // client_tx.send(Some(serde_json::to_string(&request).unwrap())).await
        //     .expect("Problem sending message.");
        //
        // let receiver_message = tokio::time::timeout(
        //     Duration::from_millis(100), connection_rx.recv()).await
        //     .expect("Timeout unwrapping receiver message.")
        //     .expect("Problem unwrapping receiver message.");
        //
        // assert_eq!(receiver_message, request);

        // assert_eq!(client_message, Some(serde_json::to_string(&request).unwrap()));
    }
}
