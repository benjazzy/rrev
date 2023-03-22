use crate::connection::ConnectionEvent;
use crate::parser::Parser;
use crate::server::server_handle::PassersServerHandle;
use std::net::SocketAddr;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc};
use tracing::warn;

pub async fn pass_messages<P: Parser>(
    mut rx: mpsc::Receiver<ConnectionEvent<P>>,
    server_hdl: PassersServerHandle<P>,
    addr: SocketAddr,
) {
    loop {
        match rx.recv().await {
            Some(event) => {
                server_hdl.new_connection_event(event, addr).await;
            }
            None => {
                warn!("Problem passing connection event to server {addr}");
                break;
            }
        }
    }
}
