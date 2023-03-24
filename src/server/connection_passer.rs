use crate::connection::ConnectionEvent;
use crate::parser::Parser;
use crate::server::server_handle::PassersServerHandle;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing::warn;

/// Passes events from a connection to a server with the connection address.
///
/// # Arguments
/// * `rx` - Receiver to listen for connection events.
/// * `server_hdl` - Server handle to pass connection events on to.
/// * `addr` - Address of the connection. Used to identify the connection event to the server.
pub async fn pass_messages<P: Parser>(
    mut rx: mpsc::Receiver<ConnectionEvent<P>>,
    server_hdl: PassersServerHandle<P>,
    addr: SocketAddr,
) {
    loop {
        match rx.recv().await {
            Some(event) => {
                if server_hdl.new_connection_event(event, addr).await.is_err() {
                    break;
                }
            }
            None => {
                warn!("Problem passing connection event to server {addr}");
                break;
            }
        }
    }
}
