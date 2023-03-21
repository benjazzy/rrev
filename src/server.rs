mod acceptor;
mod listener;
mod server_handle;
mod upgrader;

use crate::connection;
use crate::parser::Parser;
pub use listener::ListenerHandle;
use server_handle::ServerMessage;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug)]
struct Server<P: Parser> {
    rx: mpsc::Receiver<ServerMessage>,

    connections: HashMap<String, connection::ConnectionHdl<P>>,

    listener_hdl: ListenerHandle,
}

impl<P: Parser> Server<P> {
    pub fn new(rx: mpsc::Receiver<ServerMessage>) -> Self {
        todo!()
    }

    pub fn run(mut self) {
        todo!()
    }
}
