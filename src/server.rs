mod acceptor;
mod connection_passer;
mod error;
mod listener;
mod server_event;
mod server_handle;

use crate::connection;
use crate::connection::ConnectionHdl;
use crate::error::RequestError;
use crate::parser::Parser;
use crate::server::acceptor::AcceptorHandle;
use crate::server::error::{ClientsError, ListenAddrError};
use crate::server::server_handle::AcceptorsServerHandle;
pub use listener::ListenerHandle;
pub use server_event::{ConnectionEvent, ConnectionRequest, ServerEvent};
pub use server_handle::ServerHandle;
use server_handle::ServerMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

/// Represents a websocket server that can handle multiple clients.
/// Server manages a [listener::Listener] and a [acceptor::Acceptor]
/// for listening for new websocket connections.
/// Server manages these connections and allows messages
/// to be sent to and from the connections.
/// A server is managed through a [ServerHandle]
#[derive(Debug)]
struct Server<P: Parser> {
    /// Receiver for handling all incoming ServerMessages from any handle.
    rx: mpsc::Receiver<ServerMessage<P>>,

    /// Used to send events like a new connection or a request from a connection.
    tx: mpsc::Sender<ServerEvent<P>>,

    /// List of connections currently connected to the server.
    /// SocketAddr contains the ip and port of the client.
    connections: HashMap<SocketAddr, ConnectionHdl<P>>,

    /// Handle of the acceptor accepting websocket connections for the server
    /// Used to close the acceptor when the server closes.
    acceptor_hdl: AcceptorHandle,

    /// Handle of the listener listening for tcp connections for the server.
    /// Used to close the listener when the server closes.
    listener_hdl: ListenerHandle,
}

impl<P: Parser> Server<P> {
    /// Creates a new server and starts its listner and acceptor.
    /// new does not start the server.
    ///
    /// # Arguments
    /// * `rx` - Receiver for listening for ServerMessages from the handler.
    /// * `message_tx` - Copy of Sender associated with rx. `message_tx` gets passed on to the servers acceptor.
    /// * `event_tx` - Sender used to send ServerEvents.
    /// * `listen_addr` - Ip and port to listen for tcp connections.
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

    /// Starts the server.
    /// Listens for incoming ServerMessages and handles them.
    /// If the server's handler is dropped then this function will exit.
    /// If the server receives a close ServerMessage then this function will exit.
    /// This function consumes self.
    pub async fn run(mut self) {
        loop {
            let message_option = self.rx.recv().await;
            if let Some(message) = message_option {
                if let ServerMessage::Close = message {
                    break;
                }
                if self.handle_message(message).await.is_break() {
                    break;
                }
            } else {
                break;
            }
        }

        // Close current connections.
        for (addr, connection_hdl) in self.connections.iter() {
            if connection_hdl.close().await.is_err() {
                warn!("Problem closing connection {addr}");
            }
        }

        if self.acceptor_hdl.close().await.is_err() {
            warn!("Problem closing acceptor.");
        }

        if self.listener_hdl.close().await.is_err() {
            warn!("Problem closing listener.");
        }
    }

    /// Gets called for every new incoming message.
    /// Either handles the message or calls a function to handle the message.
    /// Returns [ControlFlow::Break] if the server should close.
    /// # Arguments
    /// * `message` - ServerMessage to handle.
    async fn handle_message(&mut self, message: ServerMessage<P>) -> ControlFlow<()>{
        match message {
            ServerMessage::Close => return ControlFlow::Break(()),
            ServerMessage::GetListenAddress(tx) => self.get_listen_address(tx).await,
            ServerMessage::SendRequest { to, tx, request } => {
                self.send_request(to, request, tx).await;
            }
            ServerMessage::SendEvent { to, event } => self.send_event(to, event).await,
            ServerMessage::GetClients(tx) => self.get_clients(tx).await,
            ServerMessage::NewConnection(connection, addr) => {
                debug!("New connection from {addr}");
                self.new_connection(connection, addr).await;
            }
            ServerMessage::ConnectionEvent { event, from } => {
                return self.connection_event(event, from).await;
            }
        };

        ControlFlow::Continue(())
    }

    /// Gets the listen address of the server's listener
    /// and sends it back to the caller with tx.
    ///
    /// # Arguments
    /// * `tx` Oneshot sender to send the address back to the caller.
    async fn get_listen_address(&self, tx: oneshot::Sender<Result<SocketAddr, ListenAddrError>>) {
        let try_addr = self.listener_hdl.get_addr().await;
        if tx.send(try_addr).is_err() {
            warn!("Problem sending listen address back to requester.");
        }
    }

    /// Gets the clients currently connected to the server and
    /// sends them to the requester with tx.
    ///
    /// # Arguments
    /// * `tx` Oneshot send to send the clients back to the caller.
    async fn get_clients(&self, tx: oneshot::Sender<Result<Vec<SocketAddr>, ClientsError>>) {
        let clients = dbg!(self
            .connections
            .keys()
            .collect::<Vec<&SocketAddr>>()
            .clone());
        let clients = clients.into_iter().copied().collect::<Vec<SocketAddr>>();
        if tx.send(Ok(clients)).is_err() {
            warn!("Problem sending clients back to requester.");
        }
    }

    /// Adds a new connections to self.connections and
    /// sends the new connection address with self.tx.
    ///
    /// # Arguments
    /// * `connection` - [ConnectionHandle] of the new connection.
    /// * `addr` - Address of the new connection.
    async fn new_connection(&mut self, connection: ConnectionHdl<P>, addr: SocketAddr) {
        if self.connections.get(&addr).is_some() {
            error!("Got new connection from already connected address: {addr}");
            return;
        }
        self.connections.insert(addr, connection);

        if self.tx.send(ServerEvent::NewConnection(addr)).await.is_err() {
            warn!("Could not send new connection to user.");
        }
    }

    /// Handles a ConnectionEvent from a connected connection.
    /// This function gets called whenever the
    /// server receives a connection's new [connection::ConnectionEvent].
    /// The [connection::ConnectionEvent] will be passed on as a
    /// [ServerEvent] including the address that the
    /// [connection::ConnectionEvent] came from.
    ///
    /// # Arguments
    /// * `event` - Event received from a connection.
    /// * `addr` - Ip and port of the connection that send the event.
    async fn connection_event(&mut self, event: connection::ConnectionEvent<P>, addr: SocketAddr) -> ControlFlow<()>{
        let server_event = match event {
            connection::ConnectionEvent::Close => {
                if self.connections.remove(&addr).is_none() {
                    warn!("Connection closed that was not in out list of connections.");
                }

                ServerEvent::ConnectionClose(addr)
            }
            connection::ConnectionEvent::EventMessage(e) => {
                ServerEvent::ConnectionEvent(ConnectionEvent {
                    from: addr,
                    event: e,
                })
            }
            connection::ConnectionEvent::RequestMessage(r) => {
                ServerEvent::ConnectionRequest(ConnectionRequest {
                    from: addr,
                    request: r,
                })
            }
        };

        if self.tx.send(server_event).await.is_err() {
            warn!("Problem sending event to user. Exiting.");
            return ControlFlow::Break(());
        }

        ControlFlow::Continue(())
    }

    /// Sends a request to a connection.
    ///
    /// # Arguments
    /// * `to` - Ip and port of the connection that the event will be sent to.
    /// * `request` - The request to send.
    ///* `tx` -  The oneshot to reply on. Gets passed to the connection to reply with.
    async fn send_request(
        &mut self,
        to: SocketAddr,
        request: P::OurRequest,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
    ) {
        if let Some(connection_hdl) = self.connections.get(&to) {
            if connection_hdl.request_with_sender(request, tx).await.is_err() {
                warn!("Problem sending message to connection {to}. Dropping.");
                self.connections.remove(&to);
            }
        } else {
            warn!("Unknown connection {to}.");
        }
    }

    /// Sends an event to a connection
    ///
    /// # Arguments
    /// * `to` - Ip and port of the connection that the event will be sent to.
    /// * `event` - Event to send.
    async fn send_event(&mut self, to: SocketAddr, event: P::OurEvent) {
        if let Some(connection_hdl) = self.connections.get(&to) {
            if connection_hdl.event(event).await.is_err() {
                warn!("Problem sending message to connection {to}. Dropping.");
                self.connections.remove(&to);
            }
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

    // Check if we can get the listen addres of a running server.
    #[tokio::test]
    async fn check_get_address() {
        let ip = "127.0.0.1";

        // Start the server.
        let (server_tx, _server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        // Get the listen address of the server.
        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());
    }

    // Check that a client can connect to the server and the server sends a new connection event.
    #[tokio::test]
    async fn check_connect() {
        let ip = "127.0.0.1";

        // Start the server.
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        // Get the listen address of the server.
        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());

        // Connect with a client and check that a client was added.
        let url =
            url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url.");
        let (client_tx, _client_rx) = mpsc::channel(1);
        let _client_hdl = connect::<StringParser>(url, client_tx).await;

        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting server message.")
            .expect("Problem getting server message.");

        // Check that the server_message was a NewConnection.
        let connection_addr = if let ServerEvent::NewConnection(addr) = server_message {
            addr
        } else {
            panic!("Did not receive new connection message from the server");
        };

        // We don't know what port the client is using however we can check that the ip is correct.
        assert_eq!(connection_addr.ip().to_string(), ip.to_string());
    }

    // Check that getting the connected clients from the server works.
    #[tokio::test]
    async fn check_get_clients() {
        let ip = "127.0.0.1";

        // Start the server
        let (server_tx, mut server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        // Get the server's listen address.
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
            url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url.");

        let (client_tx, _client_rx) = mpsc::channel(1);

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

        // Check that we receive a new connection event from the server.
        let server_message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting server message.")
            .expect("Problem getting server message.");

        let connection_addr = if let ServerEvent::NewConnection(addr) = server_message {
            addr
        } else {
            panic!("Did not receive new connection message from the server");
        };

        assert_eq!(connection_addr.ip().to_string(), ip.to_string());

        // Close the client and check that the server says there are no clients connected.
        connection_hdl.close().await.expect("Problem closing connection.");
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

    // Check that the server is able to send a request to a client.
    #[tokio::test]
    async fn check_send_request() {
        let request = "test request";
        let test_reply = "test reply";
        let ip = "127.0.0.1";

        // Start the server.
        let (server_tx, _server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        // Get the listen address of the server.
        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());

        // Connect with a client and check that a client was added.
        let url =
            url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url.");

        let (client_tx, client_rx) = mpsc::channel(1);

        let _connection_hdl = connect::<StringParser>(url, client_tx)
            .await
            .expect("Problem connecting to server.");

        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients.len(), 1);
        let first = clients.get(0).expect("Problem getting first client.");
        assert_eq!(first.ip().to_string(), ip.to_string());

        // Allows the client to reply to the request in the background.
        let reply_closure = async move |mut client_rx: mpsc::Receiver<ConnectionEvent<StringParser>>| {
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
            client_request.complete(test_reply.to_string()).await.expect("Problem completing request.");
        };

        // Start the client listening for and replying to the request in the background.
        tokio::spawn(reply_closure(client_rx));

        // Send the request to the client.
        let server_reply = server_hdl
            .request_timeout(*first, request.to_string(), Duration::from_millis(100))
            .await
            .expect("Problem getting reply");

        // Check the reply.
        assert_eq!(server_reply, test_reply.to_string());
    }

    // Check that the server is able to send an event.
    #[tokio::test]
    async fn check_send_event() {
        let event = "test event";
        let ip = "127.0.0.1";

        // Start the server.
        let (server_tx, _server_rx) = mpsc::channel(1);
        let server_hdl =
            ServerHandle::<StringParser>::new(format!("{ip}:0").to_string(), server_tx)
                .await
                .expect("Problem starting server");

        // Get the listen address of the server.
        let address =
            tokio::time::timeout(Duration::from_millis(100), server_hdl.get_listen_address())
                .await
                .expect("Timeout getting listen address.")
                .expect("Problem receiving listen address.");

        assert_eq!(address.ip().to_string(), ip.to_string());

        // Connect with a client and check that a client was added.
        let url =
            url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url.");

        let (client_tx, mut client_rx) = mpsc::channel(1);

        let _connection_hdl = connect::<StringParser>(url, client_tx)
            .await
            .expect("Problem connecting to server.");

        let clients = server_hdl
            .get_clients()
            .await
            .expect("Problem getting clients.");
        assert_eq!(clients.len(), 1);
        let first = clients.get(0).expect("Problem getting first client.");
        assert_eq!(first.ip().to_string(), ip.to_string());

        // Send event.
        server_hdl.event(*first, event.to_string()).await.expect("Problem sending event");

        let client_message = tokio::time::timeout(Duration::from_millis(100), client_rx.recv())
            .await
            .expect("Timeout receiving event from the server")
            .expect("Problem receiving event from the server");

        let client_event = if let ConnectionEvent::EventMessage(event) = client_message {
            event
        } else {
            panic!("Server sent incorrect event");
        };

        assert_eq!(client_event, event.to_string());
    }
}
