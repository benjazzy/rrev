mod listener_handle;

use std::net::SocketAddr;
use std::ops::ControlFlow;
use tokio::{io, net};
use tokio::sync::mpsc;
use tracing::{debug, warn};

pub use listener_handle::ListenerHandle;
use listener_handle::ListenerMessage;

#[derive(Debug)]
struct Listener {
    rx: mpsc::Receiver<ListenerMessage>,
    socket: net::TcpListener,
}

impl Listener {
    pub async fn new(rx: mpsc::Receiver<ListenerMessage>, addr: String) -> io::Result<Self>  {
        let socket = net::TcpListener::bind(addr).await?;
        Ok(Listener { rx, socket })
    }

    pub async fn run(mut self) {
        loop {
            let control = tokio::select! {
                Some(message) = self.rx.recv() => {
                    self.handle_message(message).await
                }
            };

            if let ControlFlow::Break(()) = control {
                break;
            }
        }
    }

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
    use std::net::SocketAddr;
    use crate::server::listener::ListenerHandle;

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
}