mod connection_hdl;
mod internal_hdl;
mod receiver;
mod sender;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, warn};

use crate::scheme;
use crate::scheme::internal;

use crate::connection::internal_hdl::{InternalHdl, InternalMessage};
use crate::scheme::RequestHandle;
pub use connection_hdl::{ConnectionHdl, RequestError};
use receiver::ReceiverHdl;
pub use sender::SenderHdl;

#[derive(Debug)]
enum ConnectionMessage<
    OurReq: Serialize,
    OurRep: Serialize,
    OurEvent: Serialize,
    TheirReq,
    TheirRep,
    TheirEvent,
> {
    Close,
    Request {
        data: OurReq,
        tx: oneshot::Sender<Result<TheirRep, RequestError>>,
    },
    Reply(OurRep),
    Event(OurEvent),
    Send(internal::Message<OurReq, OurRep, OurEvent>),
    RequestListener(mpsc::Sender<RequestHandle<OurReq, OurRep, OurEvent, TheirReq>>),
    EventListener(mpsc::Sender<TheirEvent>),
}

struct Connection<
    OurReq: Serialize,
    OurRep: Serialize,
    OurEvent: Serialize,
    TheirReq: for<'a> Deserialize<'a>,
    TheirRep: for<'a> Deserialize<'a>,
    TheirEvent: for<'a> Deserialize<'a>,
> {
    next_id: usize,

    receiver_hdl: ReceiverHdl,
    sender_hdl: SenderHdl<OurReq, OurRep, OurEvent>,

    internal_rx: mpsc::Receiver<InternalMessage<TheirReq, TheirRep, TheirEvent>>,
    rx: mpsc::Receiver<ConnectionMessage<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>>,

    reply_map: HashMap<usize, oneshot::Sender<Result<TheirRep, RequestError>>>,
    request_listeners: Vec<mpsc::Sender<RequestHandle<OurReq, OurRep, OurEvent, TheirReq>>>,
    event_listeners: Vec<mpsc::Sender<TheirEvent>>,
}

impl<
        OurReq: Serialize + Send + 'static,
        OurRep: Serialize + Send + 'static,
        OurEvent: Serialize + Send + 'static,
        TheirReq: for<'a> Deserialize<'a> + Clone + Send + 'static,
        TheirRep: for<'a> Deserialize<'a> + Clone + Send + 'static,
        TheirEvent: for<'a> Deserialize<'a> + Clone + Send + 'static,
    > Connection<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>
{
    pub fn new(
        rx: mpsc::Receiver<
            ConnectionMessage<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>,
        >,
        stream: WebSocketStream<TcpStream>,
    ) -> Self {
        let (internal_tx, internal_rx) = mpsc::channel(1);
        let internal_hdl: InternalHdl<TheirReq, TheirRep, TheirEvent> =
            InternalHdl::new(internal_tx);
        let (sender, receiver) = stream.split();

        let sender_hdl = SenderHdl::new(internal_hdl.clone(), sender);
        let receiver_hdl = ReceiverHdl::new(internal_hdl.clone(), receiver);

        Connection {
            next_id: 0,

            receiver_hdl,
            sender_hdl,

            internal_rx,
            rx,

            reply_map: HashMap::new(),
            request_listeners: vec![],
            event_listeners: vec![],
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
        self.cancel_requests().await
    }

    async fn handle_external(
        &mut self,
        message: ConnectionMessage<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>,
    ) -> ControlFlow<()> {
        match message {
            ConnectionMessage::Event(event) => {
                self.sender_hdl.send(internal::Message::Event(event)).await;
            }
            ConnectionMessage::Request { data, tx } => {
                self.send_request(data, tx).await;
            }
            ConnectionMessage::Reply(reply) => {
                todo!()
            }
            ConnectionMessage::Close => {
                return ControlFlow::Break(());
            }
            ConnectionMessage::Send(m) => {
                self.sender_hdl.send(m).await;
            }
            ConnectionMessage::RequestListener(request_listener) => {
                self.external_new_request_handler(request_listener);
            }
            ConnectionMessage::EventListener(event_listener) => {
                self.event_listeners.push(event_listener);
            }
        }

        ControlFlow::Continue(())
    }

    async fn handle_internal(
        &mut self,
        message: InternalMessage<TheirReq, TheirRep, TheirEvent>,
    ) -> ControlFlow<()> {
        match message {
            InternalMessage::Close => {
                return ControlFlow::Break(());
            }
            InternalMessage::NewMessage(message) => {
                self.handle_internal_new_message(message).await;
            }
        }

        ControlFlow::Continue(())
    }

    fn external_new_request_handler(
        &mut self,
        hdl: mpsc::Sender<RequestHandle<OurReq, OurRep, OurEvent, TheirReq>>,
    ) {
        println!("new request handler");
        self.request_listeners.push(hdl);
    }

    async fn handle_internal_new_message(
        &mut self,
        message: internal::Message<TheirReq, TheirRep, TheirEvent>,
    ) {
        match message {
            internal::Message::Request(request) => self.handle_internal_request(request).await,
            internal::Message::Reply(reply) => self.handle_internal_reply(reply).await,
            internal::Message::Event(event) => {
                self.handle_internal_event(event).await;
            }
        }
    }

    async fn handle_internal_request(&mut self, request: internal::Request<TheirReq>) {
        for request_listener in self.request_listeners.iter() {
            let handle = RequestHandle::new(request.clone(), self.sender_hdl.clone());

            request_listener.send(handle).await;
        }
    }

    async fn handle_internal_reply(&mut self, reply: internal::Reply<TheirRep>) {
        if let Some(tx) = self.reply_map.remove(&reply.id) {
            if let Err(_) = tx.send(Ok(reply.data)) {
                warn!("Problem sending reply back to requester");
            };
        } else {
            warn!("No request id matches reply id.");
        };
    }

    async fn handle_internal_event(&self, event: TheirEvent) {
        for tx in self.event_listeners.iter() {
            let event = event.clone();
            let _ = tx.send(event).await;
        }
    }

    async fn send_request(
        &mut self,
        data: OurReq,
        tx: oneshot::Sender<Result<TheirRep, RequestError>>,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        if self.reply_map.contains_key(&id) {
            warn!("Failed to send request. Request id has already been used.");
            return;
        }
        self.reply_map.insert(id, tx);

        let request = internal::Message::Request(internal::Request::<OurReq> { id, data });
        self.sender_hdl.send(request).await;
    }

    async fn cancel_requests(&mut self) {
        let keys: Vec<usize> = self.reply_map.iter().map(|(key, _)| *key).collect();

        for key in keys.iter() {
            if let Some(tx) = self.reply_map.remove(key) {
                let _ = tx.send(Err(RequestError::Closed));
            }
        }
    }

    async fn close(&mut self) {
        self.receiver_hdl.close().await;
        self.sender_hdl.close().await;
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::connection_hdl::RequestError;
    use crate::connection::ConnectionHdl;
    use crate::scheme::internal;
    use futures_util::stream::{SplitSink, SplitStream};
    use futures_util::{SinkExt, StreamExt};
    use std::collections::HashMap;
    use std::io::BufRead;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;
    use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};

    type ConHdlType = ConnectionHdl<String, String, String, String, String, String>;

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
        if let Some(message) = read.next().await {
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
                    tx.send(m.to_string()).await;
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
        mut rx: Option<mpsc::Receiver<String>>,
    ) {
        let ws_stream = client(addr).await;

        let (mut write, mut read) = ws_stream.split();

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
            match message {
                Ok(m) => {
                    assert!(m.is_text());
                    let m_str = m.to_text().unwrap();
                    assert!(responses.contains_key(m_str));
                    send.send(tungstenite::Message::Text(
                        responses.get(m_str).unwrap().clone(),
                    ))
                    .await
                    .unwrap();
                }
                Err(e) => {}
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

    async fn server(socket: TcpListener) -> ConHdlType {
        let (stream, _) = socket.accept().await.expect("Error accepting connection.");

        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error accepting websocket stream.");

        let connection_hdl = ConHdlType::new(ws_stream).await;

        connection_hdl
    }

    #[tokio::test]
    async fn check_timeout() {
        let message = "test";
        let request = internal::Message::<String, String, String>::Request(internal::Request {
            id: 0,
            data: message.to_string(),
        });

        let (client_tx, mut client_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, Some(client_tx), None));
        let connection_hdl = server(socket).await;

        let timeout = connection_hdl
            .request_timeout(message.to_string(), Duration::from_millis(10))
            .await;

        // Assert that the request timed out.
        assert_eq!(Err(RequestError::Timeout), timeout);

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
        let connection_hdl = server(socket).await;

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
        let connection_hdl = server(socket).await;

        connection_hdl.event(message.to_string()).await;

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

        let (client_tx, mut client_rx) = mpsc::channel(1);
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, None, Some(client_rx)));
        let connection_hdl = server(socket).await;

        connection_hdl.register_event_listener(server_tx).await;
        client_tx.send(event_str).await;

        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting message.")
            .expect("Empty message");

        assert_eq!(server_message, message);
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

        let (our_client_tx, mut client_rx) = mpsc::channel(1);
        let (client_tx, mut our_client_rx) = mpsc::channel(1);
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (socket, addr) = socket().await;
        tokio::spawn(send_client(addr, Some(client_tx), Some(client_rx)));
        let connection_hdl = server(socket).await;

        connection_hdl.register_request_listener(server_tx).await;

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

        server_message.complete(message.to_string()).await;

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
        let connection_hdl = server(socket).await;

        let close = connection_hdl
            .request_timeout(message.to_string(), Duration::from_millis(100))
            .await;

        // Assert that the request failed because of a close.
        assert_eq!(Err(RequestError::Closed), close);
    }
}
