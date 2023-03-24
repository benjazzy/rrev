mod listener_handle;

use std::net::SocketAddr;
use std::ops::ControlFlow;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::{io, net};
use tracing::{debug, warn};

use crate::server::acceptor::ListenersAcceptorHandle;
pub use listener_handle::ListenerHandle;
use listener_handle::ListenerMessage;

/// Listens for incoming connections.
#[derive(Debug)]
struct Listener {
    /// Listens for messages from the handler.
    rx: mpsc::Receiver<ListenerMessage>,

    /// Allows the listener to pass tcp connections on to the acceptor.
    acceptor_hdl: ListenersAcceptorHandle,

    /// Tcp socket to listen for connections on.
    socket: net::TcpListener,
}

impl Listener {
    /// Creates a new listener.
    /// If an invalid listen address is passed in, return
    /// the associated tokio:io error.
    ///
    /// # Arguments
    /// * `rx` - Receiver to listen for ListenerMessages on.
    /// * `tx` - Sender to send new tcp connections to.
    /// * `addr` - Address to listen for connection on.
    pub async fn new(
        rx: mpsc::Receiver<ListenerMessage>,
        acceptor_hdl: ListenersAcceptorHandle,
        addr: String,
    ) -> io::Result<Self> {
        let socket = net::TcpListener::bind(addr).await?;
        Ok(Listener {
            rx,
            acceptor_hdl,
            socket,
        })
    }

    /// Starts listening for connections and ListenerMessages.
    /// Will run until the handler closes,
    /// the self.tx closes, or
    /// it receives a close ListenerMessage.
    pub async fn run(mut self) {
        loop {
            let control = tokio::select! {
                message_option = self.rx.recv() => {
                    if let Some(message) = message_option {
                        self.handle_message(message).await
                    } else {
                        ControlFlow::Break(())
                    }
                }
                stream_result = self.socket.accept() => {
                    match stream_result {
                        Ok((stream, addr)) => {
                            debug!("New tcp connection from {addr}");
                            self.handle_new_stream(stream, addr).await
                        },
                        Err(e) => {
                            warn!("Problem accepting new tcp connection. {e}");
                            ControlFlow::Continue(())
                        }
                    }
                }
            };

            if let ControlFlow::Break(()) = control {
                break;
            }
        }

        debug!("Listener is closing.");
    }

    /// Gets called for every incoming ListenerMessage
    ///
    /// # Arguments
    /// * `message` - Listener message that was received.
    async fn handle_message(&self, message: ListenerMessage) -> ControlFlow<()> {
        match message {
            ListenerMessage::Close => {
                return ControlFlow::Break(());
            }
            ListenerMessage::GetAddress(tx) => {
                let _ = tx.send(self.socket.local_addr());
            }
        }

        ControlFlow::Continue(())
    }

    /// Passes a new stream on to our acceptor handle.
    ///
    /// # Arguments
    /// * `stream` - The new tcp stream.
    /// * `addr` - The address of the new tcp stream.
    async fn handle_new_stream(&self, stream: TcpStream, addr: SocketAddr) -> ControlFlow<()> {
        if self.acceptor_hdl.new_stream(stream, addr).await.is_err() {
            warn!("Problem sending new stream to acceptor. Exiting.");
            return ControlFlow::Break(())
        }

        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use crate::server::acceptor::{AcceptorHandle, AcceptorMessage};
    use crate::server::listener::ListenerHandle;
    use std::assert_matches::assert_matches;
    use std::str;
    use std::time::Duration;
    use tokio::io;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::sync::mpsc;

    // Checks for ListenerHandle's ability to get the address from its running Listener.
    #[tokio::test]
    async fn check_get_addr() {
        let (tx, _rx) = mpsc::channel(1);
        let acceptor_hdl = AcceptorHandle::from_tx(tx);

        let address = "127.0.0.1";
        let port = 0;

        let listener_handle = ListenerHandle::new(acceptor_hdl.into(), format!("{address}:{port}"))
            .await
            .expect("Problem creating listener");

        let addr = listener_handle
            .get_addr()
            .await
            .expect("Problem getting listen address.");

        assert_eq!(addr.ip().to_string(), address);
    }

    // Checks that if we give Listener a bad address to listen on it will return an error.
    #[tokio::test]
    async fn check_bad_addr() {
        let (tx, _rx) = mpsc::channel(1);
        let acceptor_hdl = AcceptorHandle::from_tx(tx);

        let bad_address = "1.1.1.1:0";
        let result = ListenerHandle::new(acceptor_hdl.into(), bad_address.to_string()).await;
        assert_matches!(result, Err(_));

        let error = result.expect_err("Result is not an error.");
        assert_matches!(error.kind(), io::ErrorKind::AddrNotAvailable);
    }

    // Check that Listener will pass new connections onto its acceptor
    #[tokio::test]
    async fn check_connect() {
        let message = "test message";
        let (tx, mut rx) = mpsc::channel(1);
        let acceptor_hdl = AcceptorHandle::from_tx(tx);
        let address = "127.0.0.1";
        let port = 0;

        let listener_handle = ListenerHandle::new(acceptor_hdl.into(), format!("{address}:{port}"))
            .await
            .expect("Problem creating listener");

        let port = listener_handle
            .get_addr()
            .await
            .expect("Problem getting listener address")
            .port();

        let mut client_stream = TcpStream::connect(format!("{address}:{port}"))
            .await
            .expect("Problem connecting to listener.");

        let mut server_stream = match tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("Timeout getting server_stream.")
            .expect("Problem getting server_stream.")
        {
            AcceptorMessage::NewStream(stream, _) => stream,
            _ => panic!("Didn't get NewStream message."),
        };

        let mut buf: [u8; 12] = [0; 12];
        client_stream
            .write_all(message.as_bytes())
            .await
            .expect("Problem sending message to server.");
        let _ = server_stream
            .read_exact(&mut buf)
            .await
            .expect("Problem reading message.");

        let string_buf = str::from_utf8(&buf).expect("Problem converting bytes to string.");
        assert_eq!(string_buf, message);
    }
}
