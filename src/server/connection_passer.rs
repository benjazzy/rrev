use crate::connection::ConnectionEvent;
use crate::parser::Parser;
use crate::server::server_handle::PassersServerHandle;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing::debug;

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
    while let Some(event) = rx.recv().await {
        if server_hdl.new_connection_event(event, addr).await.is_err() {
            break;
        }
    }

    debug!("Passer exiting {addr}.");
}
