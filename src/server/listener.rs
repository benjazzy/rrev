mod listener_handle;

use std::net::SocketAddr;
use std::ops::ControlFlow;
use tokio::{io, net};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, warn};

pub use listener_handle::{ ListenerHandle, Error };
use listener_handle::ListenerMessage;
use crate::sender_manager::SenderManager;

/// Listens for incoming connections.
#[derive(Debug)]
struct Listener {
    /// Listens for messages from the handler.
    rx: mpsc::Receiver<ListenerMessage>,

    /// List of senders to notify when we get a new tcp connection.
    listeners: SenderManager<mpsc::Sender<TcpStream>>,

    // Tcp socket to listen for connections on.
    socket: net::TcpListener,
}

impl Listener {
    /// Creates a new listener.
    /// If an invalid listen address is passed in, return
    /// the associated tokio:io error.
    ///
    /// # Arguments
    /// * `rx` - Receiver to listen for ListenerMessages on.
    /// * `addr` - Address to listen for connection on.
    pub async fn new(rx: mpsc::Receiver<ListenerMessage>, addr: String) -> io::Result<Self>  {
        let socket = net::TcpListener::bind(addr).await?;
        Ok(Listener {
            rx,
            listeners: SenderManager::new(),
            socket
        })
    }

    /// Starts listening for connections and ListenerMessages.
    /// Will run until the handler closes or it receives a close ListenerMessage.
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
            };

            if let ControlFlow::Break(()) = control {
                break;
            }
        }
    }

    pub fn register_connection_listener(&mut self, tx: mpsc::Sender<TcpStream>) -> usize {
        self.listeners.add(tx)
    }

    /// Gets called for every incoming ListenerMessage
    ///
    /// # Arguments
    /// * `message` - Listener message that was received.
    async fn handle_message(&self, message: ListenerMessage) -> ControlFlow<()> {
        match message {
            ListenerMessage::Close => {
                debug!("Listener is closing.");
                return ControlFlow::Break(());
            }
            ListenerMessage::GetAddress(tx) => {
                let _ = tx.send(self.socket.local_addr());
            }
        }

        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::io;
    use crate::server::listener::ListenerHandle;

    // Checks for ListenerHandle's ability to get the address from its running Listener.
    #[tokio::test]
    async fn check_get_addr() {
        let address = "127.0.0.1";
        let port = 0;

        let listener_handle =
            ListenerHandle::new(format!("{address}:{port}")).await
                .expect("Problem creating listener");

        let addr = listener_handle.get_addr().await
            .expect("Problem getting listen address.");

        assert_eq!(addr.ip().to_string(), address);
    }

    // Checks that if we give Listener a bad address to listen on it will return an error.
    #[tokio::test]
    async fn check_bad_addr() {
        {
            let bad_address = "1.1.1.1:0";
            let result = ListenerHandle::new(bad_address.to_string()).await;
            assert_matches!(result, Err(_));

            let error = result.expect_err("Result is not an error.");
            assert_matches!(error.kind(), io::ErrorKind::AddrNotAvailable);
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}