use crate::parser::Parser;
use crate::server::Server;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ServerMessage {
    Close,
}

#[derive(Debug, Clone)]
pub struct ServerHandle {
    tx: mpsc::Sender<ServerMessage>,
}

impl ServerHandle {
    pub fn new<P: Parser>() -> Self {
        let (tx, rx) = mpsc::channel(1);

        let server = Server::<P>::new(rx);
        server.run();

        ServerHandle { tx }
    }
}
