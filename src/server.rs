mod acceptor;
mod connection_passer;
mod error;
mod listener;
mod server_event;
mod server_handle;
mod upgrader;

use crate::connection;
use crate::connection::{ConnectionEvent, ConnectionHdl};
use crate::parser::Parser;
use crate::server::acceptor::AcceptorHandle;
use crate::server::server_event::ServerEvent;
use crate::server::server_handle::AcceptorsServerHandle;
pub use error::Error;
pub use listener::ListenerHandle;
use server_handle::ServerMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, warn};
use crate::request_error::RequestError;

#[derive(Debug)]
struct Server<P: Parser> {
    rx: mpsc::Receiver<ServerMessage<P>>,

    tx: mpsc::Sender<ServerEvent<P>>,

    connections: HashMap<SocketAddr, connection::ConnectionHdl<P>>,

    acceptor_hdl: AcceptorHandle,

    listener_hdl: ListenerHandle,
}

impl<P: Parser> Server<P> {
    pub async fn new(
        rx: mpsc::Receiver<ServerMessage<P>>,
        message_tx: mpsc::Sender<ServerMessage<P>>,
        event_tx: mpsc::Sender<ServerEvent<P>>,
        listen_addr: String,
    ) -> tokio::io::Result<Self> {
        let acceptors_server_hdl = AcceptorsServerHandle::new(message_tx);
        let acceptor_hdl = AcceptorHandle::new(acceptors_server_hdl);
        let listener_hdl = ListenerHandle::new(acceptor_hdl.clone().into(), listen_addr).await?;

        Ok(Server {
            rx,
            tx: event_tx,
            connections: HashMap::new(),
            acceptor_hdl,
            listener_hdl,
        })
    }

    pub async fn run(mut self) {
        loop {
            let message_option = self.rx.recv().await;
            if let Some(message) = message_option {
                if let ServerMessage::Close = message {
                    break;
                }
                self.handle_message(message).await;
            } else {
                break;
            }
        }
    }

    async fn handle_message(&mut self, message: ServerMessage<P>) {
        match message {
            ServerMessage::Close => todo!(),
            ServerMessage::GetListenAddress(tx) => self.get_listen_address(tx).await,
            ServerMessage::SendRequest { to, tx, request } => {
                self.send_request(to, request, tx).await;
            }
            ServerMessage::SendEvent { .. } => todo!(),
            ServerMessage::GetClients(tx) => self.get_clients(tx).await,
            ServerMessage::NewConnection(connection, addr) => {
                self.new_connection(connection, addr).await
            }
            ServerMessage::ConnectionEvent { event, from } => {
                self.connection_event(event, from).await;
            }
        };
    }

    async fn get_listen_address(&self, tx: oneshot::Sender<Result<SocketAddr, Error>>) {
        let try_addr = self.listener_hdl.get_addr().await;
        if let Err(_) = tx.send(try_addr) {
            warn!("Problem sending listen address back to requester.");
        };
    }

    async fn get_clients(&self, tx: oneshot::Sender<Result<Vec<SocketAddr>, Error>>) {
        let clients = dbg!(self
            .connections
            .keys()
            .collect::<Vec<&SocketAddr>>()
            .clone());
        let clients = clients.into_iter().map(|c| c.clone()).collect();
        if let Err(_) = tx.send(Ok(clients)) {
            warn!("Problem sending clients back to requester.");
        }
    }

    async fn new_connection(&mut self, connection: ConnectionHdl<P>, addr: SocketAddr) {
        if let Some(_) = self.connections.get(&addr) {
            error!("Got new connection from already connected address: {addr}");
            return;
        }

        self.connections.insert(addr, connection);
    }

    async fn connection_event(&mut self, event: ConnectionEvent<P>, addr: SocketAddr) {
        let server_event = match event {
            ConnectionEvent::Close => {
                if let None = self.connections.remove(&addr) {
                    warn!("Connection closed that was not in out list of connections.");
                }

                ServerEvent::ConnectionClose(addr)
            }
            ConnectionEvent::EventMessage(e) => {
                ServerEvent::ConnectionEvent(server_event::ConnectionEvent {
                    from: addr,
                    event: e,
                })
            }
            ConnectionEvent::RequestMessage(r) => {
                ServerEvent::ConnectionRequest(server_event::ConnectionRequest {
                    from: addr,
                    request: r,
                })
            }
        };

        self.tx.send(server_event).await;
    }

    async fn send_request(&self, to: SocketAddr, request: P::OurRequest, tx: oneshot::Sender<Result<P::TheirReply, RequestError>>) {
        if let Some(connection_hdl) = self.connections.get(&to) {
            connection_hdl.request_with_sender(request, tx).await;
        } else {
            warn!("Unknown connection {to}.");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::client::connect;
    use crate::connection::ConnectionEvent;
    use crate::parser::StringParser;
    use crate::server::server_event::ServerEvent;
    use crate::server::server_handle::ServerHandle;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn check_get_address() {
        let ip = "127.0.0.1";

        let (server_tx, _server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());
    }

    #[tokio::test]
    async fn check_get_clients() {
        let ip = "127.0.0.1";
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());

        // Check that the server clients are empty to start.
        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients, Vec::<SocketAddr>::new());

        // Connect with a client and check that a client was added.
        let url =
            url::Url::parse(format!("ws://{}", address).as_str()).expect("Problem parsing url.");

        let (client_tx, mut client_rx) = mpsc::channel(1);

        let connection_hdl = connect::<StringParser>(url, client_tx)
            .await
            .expect("Problem connecting to server.");

        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients.len(), 1);
        let first = clients.get(0).expect("Problem getting first client.");
        assert_eq!(first.ip().to_string(), ip.to_string());

        connection_hdl.close().await;
        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout receiving message from server.")
            .expect("Problem receiving message from server.");

        if let ServerEvent::ConnectionClose(_) = server_message {
        } else {
            panic!("Did not receive a connection close message from server.");
        }

        // Check that the server clients are empty after client close.
        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients, Vec::<SocketAddr>::new());
    }

    #[tokio::test]
    async fn check_send_request() {
        let request = "test request";
        let test_reply = "test reply";
        let ip = "127.0.0.1";
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());

        // Connect with a client and check that a client was added.
        let url =
            url::Url::parse(format!("ws://{}", address).as_str()).expect("Problem parsing url.");

        let (client_tx, mut client_rx) = mpsc::channel(1);

        let connection_hdl = connect::<StringParser>(url, client_tx)
            .await
            .expect("Problem connecting to server.");

        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients.len(), 1);
        let first = clients.get(0).expect("Problem getting first client.");
        assert_eq!(first.ip().to_string(), ip.to_string());

        let reply = async move |mut client_rx: mpsc::Receiver<ConnectionEvent<StringParser>>| {
            let client_event = tokio::time::timeout(Duration::from_millis(100), client_rx.recv())
                .await
                .expect("Timeout receiving")
                .expect("Problem receiving request on client");

            let client_request = if let ConnectionEvent::RequestMessage(r) = client_event {
                r
            } else {
                panic!("Server sent incorrect event type.");
            };

            assert_eq!(*client_request.get_request(), request.to_string());
            client_request.complete(test_reply.to_string()).await;
        };

        tokio::spawn(reply(client_rx));

        let server_reply = server_hdl
            .request_timeout(*first, request.to_string(), Duration::from_millis(100))
            .await
            .expect("Problem getting reply");

        assert_eq!(server_reply, test_reply.to_string());
    }
}
