mod connection_hdl;
mod event;
mod internal_hdl;
mod receiver;
mod sender;

use futures_util::StreamExt;
use std::collections::HashMap;
use std::ops::ControlFlow;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, warn};

use crate::scheme::internal;

use crate::connection::internal_hdl::{InternalHdl, InternalMessage};
pub use crate::error::RequestError;
use crate::parser::Parser;
use crate::scheme::RequestHandle;
pub use connection_hdl::ConnectionHdl;
pub use event::ConnectionEvent;
use receiver::ReceiverHdl;
pub use sender::SenderHdl;

#[derive(Debug)]
enum ConnectionMessage<P: Parser> {
    Close,
    Request {
        data: P::OurRequest,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
    },
    Event(P::OurEvent),
}

struct Connection<P: Parser> {
    receiver_hdl: ReceiverHdl,
    sender_hdl: SenderHdl<P>,

    internal_rx: mpsc::Receiver<InternalMessage<P>>,
    rx: mpsc::Receiver<ConnectionMessage<P>>,

    next_id: usize,
    reply_map: HashMap<usize, oneshot::Sender<Result<P::TheirReply, RequestError>>>,

    event_tx: mpsc::Sender<ConnectionEvent<P>>,
}

impl<P: Parser> Connection<P> {
    pub fn new(
        rx: mpsc::Receiver<ConnectionMessage<P>>,
        stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        event_tx: mpsc::Sender<ConnectionEvent<P>>,
    ) -> Self {
        let (internal_tx, internal_rx) = mpsc::channel(1);
        let internal_hdl: InternalHdl<P> = InternalHdl::new(internal_tx);
        let (sender, receiver) = stream.split();

        let sender_hdl = SenderHdl::new(internal_hdl.clone(), sender);
        let receiver_hdl = ReceiverHdl::new(internal_hdl, receiver);

        Connection {
            next_id: 0,

            receiver_hdl,
            sender_hdl,

            internal_rx,
            rx,

            reply_map: HashMap::new(),
            event_tx,
        }
    }

    pub async fn run(mut self) {
        loop {
            let control = tokio::select! {
                Some(message) = self.rx.recv() => {
                    self.handle_external(message).await
                },
                Some(message) = self.internal_rx.recv() => {
                    self.handle_internal(message).await
                },
            };

            // If we got a close message from either an internal or external message then close.
            if let ControlFlow::Break(()) = control {
                break;
            }
        }

        debug!("Connection closing.");
        self.close().await;
        self.cancel_requests().await;
        if self.event_tx.send(ConnectionEvent::Close).await.is_err() {
            warn!("Could not notify of connection close.");
        }
    }

    async fn handle_external(&mut self, message: ConnectionMessage<P>) -> ControlFlow<()> {
        match message {
            ConnectionMessage::Event(event) => {
                if self
                    .sender_hdl
                    .send(internal::Message::Event(event))
                    .await
                    .is_ok()
                {
                    ControlFlow::Continue(())
                } else {
                    warn!("Problem sending websocket message. Exiting.");

                    ControlFlow::Break(())
                }
            }
            ConnectionMessage::Request { data, tx } => self.send_request(data, tx).await,
            ConnectionMessage::Close => ControlFlow::Break(()),
        }
    }

    async fn handle_internal(&mut self, message: InternalMessage<P>) -> ControlFlow<()> {
        match message {
            InternalMessage::Close => ControlFlow::Break(()),
            InternalMessage::NewMessage(message) => self.handle_internal_new_message(message).await,
        }
    }

    async fn handle_internal_new_message(
        &mut self,
        message: internal::Message<P::TheirRequest, P::TheirReply, P::TheirEvent>,
    ) -> ControlFlow<()> {
        match message {
            internal::Message::Request(request) => self.handle_internal_request(request).await,
            internal::Message::Reply(reply) => {
                self.handle_internal_reply(reply).await;

                ControlFlow::Continue(())
            }
            internal::Message::Event(event) => self.handle_internal_event(event).await,
        }
    }

    async fn handle_internal_request(
        &self,
        request: internal::Request<P::TheirRequest>,
    ) -> ControlFlow<()> {
        let handle = RequestHandle::new(request, self.sender_hdl.clone());
        if self
            .event_tx
            .send(ConnectionEvent::RequestMessage(handle))
            .await
            .is_err()
        {
            warn!("Problem sending request event. Exiting.");
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }

    async fn handle_internal_reply(&mut self, reply: internal::Reply<P::TheirReply>) {
        if let Some(tx) = self.reply_map.remove(&reply.id) {
            if tx.send(Ok(reply.data)).is_err() {
                warn!("Problem sending reply back to requester");
            }
        } else {
            warn!("No request id matches reply id.");
        };
    }

    async fn handle_internal_event(&self, event: P::TheirEvent) -> ControlFlow<()> {
        if self
            .event_tx
            .send(ConnectionEvent::EventMessage(event))
            .await
            .is_err()
        {
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }

    async fn send_request(
        &mut self,
        data: P::OurRequest,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
    ) -> ControlFlow<()> {
        let id = self.next_id;
        self.next_id += 1;
        if self.reply_map.contains_key(&id) {
            warn!("Failed to send request. Request id has already been used.");
            return ControlFlow::Continue(());
        }
        self.reply_map.insert(id, tx);

        let request = internal::Message::Request(internal::Request::<P::OurRequest> { id, data: internal::RequestType::User(data) });
        if self.sender_hdl.send(request).await.is_err() {
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }

    async fn cancel_requests(&mut self) {
        let keys: Vec<usize> = self.reply_map.keys().copied().collect();

        for key in keys.iter() {
            if let Some(tx) = self.reply_map.remove(key) {
                let _ = tx.send(Err(RequestError::Canceled));
            }
        }
    }

    async fn close(&mut self) {
        let _ = self.receiver_hdl.close().await;
        let _ = self.sender_hdl.close().await;
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::{ConnectionEvent, ConnectionHdl};
    use crate::error::{RequestError, TimeoutError};
    use crate::parser::StringParser;
    use crate::scheme::internal;
    use futures_util::stream::{SplitSink, SplitStream};
    use futures_util::{SinkExt, StreamExt};
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;
    use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};

    type ConHdlType = ConnectionHdl<StringParser>;

    async fn client(addr: String) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let url = url::Url::parse(format!("ws://{addr}").as_str()).expect("Error parsing url.");

        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Error connecting to the server.");

        ws_stream
    }

    async fn close_client(addr: String) {
        let ws_stream = client(addr).await;

        let (mut write, mut read) = ws_stream.split();
        if let Some(_message) = read.next().await {
            write.close().await.expect("Problem closing connection");
        }
    }

    async fn send_back(
        tx: mpsc::Sender<String>,
        mut read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) {
        while let Some(message) = read.next().await {
            match message {
                Ok(m) => {
                    tx.send(m.to_string())
                        .await
                        .expect("Problem sending message back.");
                }
                Err(_) => break,
            }
        }
    }

    async fn send_to(
        mut rx: mpsc::Receiver<String>,
        mut write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>,
    ) {
        while let Some(message) = rx.recv().await {
            write
                .send(tungstenite::Message::Text(message))
                .await
                .expect("Problem sending message to server.");
        }
    }

    async fn send_client(
        addr: String,
        tx: Option<mpsc::Sender<String>>,
        rx: Option<mpsc::Receiver<String>>,
    ) {
        let ws_stream = client(addr).await;

        let (write, read) = ws_stream.split();

        if let Some(tx) = tx {
            tokio::spawn(send_back(tx, read));
        }

        if let Some(rx) = rx {
            tokio::spawn(send_to(rx, write));
        }
    }

    async fn responsive_client(addr: String, responses: HashMap<String, String>) {
        let ws_stream = client(addr).await;
        let (mut send, mut read) = ws_stream.split();

        while let Some(message) = read.next().await {
            if let Ok(m) = message {
                assert!(m.is_text());
                let m_str = m.to_text().unwrap();
                assert!(responses.contains_key(m_str));
                send.send(tungstenite::Message::Text(
                    responses.get(m_str).unwrap().clone(),
                ))
                .await
                .unwrap();
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

    async fn server(
        socket: TcpListener,
        tx: mpsc::Sender<ConnectionEvent<StringParser>>,
    ) -> ConHdlType {
        let (stream, _) = socket.accept().await.expect("Error accepting connection.");

        let maybe_tls = MaybeTlsStream::Plain(stream);

        let ws_stream = tokio_tungstenite::accept_async(maybe_tls)
            .await
            .expect("Error accepting websocket stream.");

        ConHdlType::new(ws_stream, tx).await
    }

    #[tokio::test]
    async fn check_timeout() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request {
            id: 0,
            data: message.to_string(),
        });

        let (client_tx, mut client_rx) = mpsc::channel(1);
        let (server_tx, _server_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, Some(client_tx), None));
        let connection_hdl = server(socket, server_tx).await;

        let timeout = connection_hdl
            .request_timeout(message.to_string(), Duration::from_millis(10))
            .await;

        // Assert that the request timed out.
        assert_eq!(Err(TimeoutError::Timeout), timeout);

        // Assert the the Connection did send the message.
        assert_eq!(
            client_rx
                .recv()
                .await
                .expect("Problem getting message from client."),
            serde_json::to_string(&request).expect("Problem serializing request.")
        );
    }

    #[tokio::test]
    async fn check_request() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request {
            id: 0,
            data: message.to_string(),
        });
        let reply = internal::Message::Reply::<String, String, String>(internal::Reply {
            id: 0,
            data: message.to_string(),
        });

        let mut requests = HashMap::new();
        requests.insert(
            serde_json::to_string(&request).unwrap(),
            serde_json::to_string(&reply).unwrap(),
        );

        let (socket, addr) = socket().await;
        tokio::spawn(responsive_client(addr, requests));
        let (server_tx, _server_rx) = mpsc::channel(1);
        let connection_hdl = server(socket, server_tx).await;

        let result = connection_hdl
            .request_timeout(message.to_string(), Duration::from_millis(100))
            .await
            .expect("Timeout getting request result");

        assert_eq!(message, result);
    }

    #[tokio::test]
    async fn check_send_event() {
        let message = "test";
        let event = internal::Message::<String, String, String>::Event(message.to_string());
        let (client_tx, mut client_rx) = mpsc::channel(1);

        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, Some(client_tx), None));
        let (server_tx, _server_rx) = mpsc::channel(1);
        let connection_hdl = server(socket, server_tx).await;

        connection_hdl
            .event(message.to_string())
            .await
            .expect("Problem sending event.");

        let client_message = tokio::time::timeout(Duration::from_millis(100), client_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message");

        assert_eq!(
            client_message.as_str(),
            serde_json::to_string(&event).unwrap()
        );
    }

    #[tokio::test]
    async fn check_recv_event() {
        let message = "test";
        let event = internal::Message::<String, String, String>::Event(message.to_string());
        let event_str = serde_json::to_string(&event).expect("Problem serializing event.");

        let (client_tx, client_rx) = mpsc::channel(1);
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, None, Some(client_rx)));

        let _connection_hdl = server(socket, server_tx).await;

        client_tx
            .send(event_str)
            .await
            .expect("Problem sending event.");

        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message");

        if let ConnectionEvent::EventMessage(m) = server_message {
            assert_eq!(m, message);
        } else {
            panic!("Server did not receive an event.");
        }
    }

    #[tokio::test]
    async fn check_recv_request() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request {
            id: 0,
            data: message.to_string(),
        });
        let request_str = serde_json::to_string(&request).expect("Problem serializing request.");

        let reply = internal::Message::<String, String, String>::Reply(internal::Reply {
            id: 0,
            data: message.to_string(),
        });
        let reply_str = serde_json::to_string(&reply).expect("Problem serializing reply.");

        let (our_client_tx, client_rx) = mpsc::channel(1);
        let (client_tx, mut our_client_rx) = mpsc::channel(1);
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, Some(client_tx), Some(client_rx)));
        let _connection_hdl = server(socket, server_tx).await;

        // Send our request to the server connection.
        our_client_tx
            .send(request_str)
            .await
            .expect("Problem sending request to client.");

        // Receive the request from the server connection.
        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message.");

        if let ConnectionEvent::RequestMessage(r) = server_message {
            r.complete(message.to_string())
                .await
                .expect("Problem completing request.");
        } else {
            panic!("Server got incorrect message type.");
        }

        let client_message = tokio::time::timeout(Duration::from_millis(100), our_client_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message.");

        assert_eq!(reply_str, client_message);
    }

    #[tokio::test]
    async fn check_close_request() {
        let message = "test";

        let (socket, addr) = socket().await;
        tokio::spawn(close_client(addr));
        let (tx, _rx) = mpsc::channel(1);
        let connection_hdl = server(socket, tx).await;

        let close = connection_hdl
            .request_timeout(message.to_string(), Duration::from_millis(100))
            .await;

        // Assert that the request failed because of a close.
        assert_eq!(
            Err(TimeoutError::RequestError(RequestError::Canceled)),
            close
        );
    }
}
