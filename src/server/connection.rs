mod connection_hdl;
mod internal_hdl;
mod scheme;
mod sender;
mod receiver;

use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::scheme::Request;

pub use connection_hdl::ConnectionHdl;

pub enum ConnectionMessage<T: Serialize> {
    Close,
    Req(Request<T>),
}

struct Connection<Req: Serialize> {
    rx: mpsc::Receiver<ConnectionMessage<Req>>,
    ws_sender: SplitSink<WebSocketStream<TcpStream>, Message>,
    ws_receiver: SplitStream<WebSocketStream<TcpStream>>
}

impl<Req: Serialize> Connection<Req> {
    pub fn new(rx: mpsc::Receiver<ConnectionMessage<Req>>, stream: WebSocketStream<TcpStream>) -> Self {
        let (ws_sender, ws_receiver) = stream.split();
        Connection { rx, ws_sender, ws_receiver }
    }

    pub async fn run(self) {

    }
}
