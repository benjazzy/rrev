mod multi_acceptor_handle;

use crate::parser::Parser;
use crate::server::acceptor::{AcceptorMessage, ListenersAcceptorHandle};
use crate::server::server_handle::AcceptorsServerHandle;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::server::ListenerHandle;
pub use multi_acceptor_handle::MultiAcceptorHandle;

struct MultiAcceptor<P: Parser> {
    rx: mpsc::Receiver<AcceptorMessage>,
    server_hdls: HashMap<String, AcceptorsServerHandle<P>>,
}

impl<P: Parser> MultiAcceptor<P> {
    pub fn new(
        rx: mpsc::Receiver<AcceptorMessage>,
        server_hdls: HashMap<String, AcceptorsServerHandle<P>>,
    ) -> Self {
        MultiAcceptor { rx, server_hdls }
    }

    pub async fn run(self) {
        todo!()
    }
}
