use std::ops::ControlFlow;
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::error;

use crate::scheme::internal::Request;
use super::internal_hdl;

pub enum SenderMessage<T> {
    Req(Request<T>),
    Close,
}

pub struct Sender<T: Serialize> {
    connection_hdl: internal_hdl::InternalHdl<(), (), ()>,
    rx: mpsc::Receiver<SenderMessage<T>>,
    ws_sender: SplitSink<WebSocketStream<TcpStream>, Message>,
}

#[derive(Clone)]
pub struct SenderHdl<T: Serialize> {
    tx: mpsc::Sender<SenderMessage<T>>,
}

impl<T: Serialize> Sender<T> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<(), (), ()>,
        rx: mpsc::Receiver<SenderMessage<T>>,
        ws_sender: SplitSink<WebSocketStream<TcpStream>, Message>
    ) -> Self
    {
        Sender { connection_hdl: connection_handle, rx, ws_sender }
    }

    pub async fn run(mut self) {
        loop {
            if let Some(request) = self.rx.recv().await {
                if let ControlFlow::Break(()) = self.handle_message(request).await {
                    break;
                };
            };
        }
    }

    async fn handle_message(&mut self, message: SenderMessage<T>) -> ControlFlow<()> {
        match message {
            SenderMessage::Req(r) => { self.send(r).await; }
            SenderMessage::Close => {
                let _ = self.ws_sender.close().await;
                return ControlFlow::Break(());
            }
        };

        ControlFlow::Continue(())
    }

    async fn send(&mut self, request: Request<T>) {
        let message_str = match serde_json::to_string(&request) {
            Ok(m) => m,
            Err(e) => {
                error!("Error serializing request! {e}");
                return;
            }
        };

        if let Err(e) = self.ws_sender.send(Message::Text(message_str)).await {

        }
    }
}

impl<T: Serialize + Send + 'static> SenderHdl<T> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<(), (), ()>,
        ws_sender: SplitSink<WebSocketStream<TcpStream>, Message>
    ) -> Self
    {
        let (tx, rx) = mpsc::channel(1);
        let sender = Sender::new(connection_handle, rx, ws_sender);

        tokio::spawn(sender.run());

        Self { tx }
    }

    pub async fn close(&self) {
        let _ = self.tx.send(SenderMessage::Close).await;
    }

    pub async fn request(&self, req: Request<T>) {
        let r = self.tx.send(SenderMessage::Req(req)).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use futures_util::StreamExt;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::connect_async;
    use crate::scheme;
    use crate::scheme::internal;
    use crate::scheme::internal::Request;
    use crate::server::connection::internal_hdl;
    use crate::server::connection::sender::SenderHdl;

    async fn client(addr: String, tx: mpsc::Sender<String>) {
        let url = url::Url::parse(format!("ws://{addr}").as_str())
            .expect("Error parsing url.");

        let (ws_stream, _) = connect_async(url).await
            .expect("Error connecting to the server.");

        let (_, mut read) = ws_stream.split();
        while let Some(message) = read.next().await {
            match message {
                Ok(m) => {
                    tx.send(m.to_string()).await;
                },
                Err(_) => break,
            }
        }
    }

    async fn socket() -> (TcpListener, String) {
        let socket = TcpListener::bind("127.0.0.1:0").await
            .expect("Error binding on socket address");
        let addr = socket.local_addr()
            .expect("Error getting socket listen address");

        (socket, addr.to_string())
    }

    async fn server(socket: TcpListener) -> SenderHdl<String> {
        let (internal_tx, internal_rx)
            = mpsc::channel(1);

        let internal_hdl = internal_hdl::InternalHdl::new(internal_tx);

        let (stream, _) = socket.accept().await
            .expect("Error accepting connection.");

        let ws_stream =
            tokio_tungstenite::accept_async(stream).await
            .expect("Error accepting websocket stream.");

        let (write, _) = ws_stream.split();

        let sender_hdl = SenderHdl::new(internal_hdl, write);

        sender_hdl
    }

    #[tokio::test]
    async fn check_sender() {
        let message = "test";
        let request = Request{ id: 0, data: message.to_string() };
        let (client_tx, mut client_rx) = mpsc::channel(1);

        let (socket, addr) = socket().await;
        tokio::spawn(client(addr, client_tx));
        let sender_hdl = server(socket).await;

        sender_hdl.request(request.clone()).await;

        let client_message =
            tokio::time::timeout(Duration::from_millis(100), client_rx.recv())
                .await.expect("Timeout getting message.")
                .expect("Empty message");

        assert_eq!(client_message.as_str(), serde_json::to_string(&request).unwrap());
    }
}
