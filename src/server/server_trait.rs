use crate::parser::Parser;
use crate::server::server_handle::ServerMessage;
use crate::server::ServerEvent;
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait Server<P: Parser>: 'static {
    const NAME: &'static str;

    fn new(rx: mpsc::Receiver<ServerMessage<P>>, event_tx: mpsc::Sender<ServerEvent<P>>) -> Self;

    async fn run(self);
}
