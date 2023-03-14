use futures_util::stream::SplitStream;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;

use super::internal_hdl;

pub enum ReceiverMessage {
    Close,
}

pub struct Receiver<
    Req: for<'a> Deserialize<'a>,
    Rep: for<'a> Deserialize<'a>,
    Event: for<'a> Deserialize<'a>
> {
    connection_handle: internal_hdl::InternalHdl<Req, Rep, Event>,
    rx: mpsc::Receiver<ReceiverMessage>,
    ws_receiver: SplitStream<WebSocketStream<TcpStream>>,
}

impl<
    Req: for<'a> Deserialize<'a>,
    Rep: for<'a> Deserialize<'a>,
    Event: for<'a> Deserialize<'a>
> Receiver<Req, Rep, Event> {
    pub fn new(
        connection_handle: internal_hdl::InternalHdl<Req, Rep, Event>,
        rx: mpsc::Receiver<ReceiverMessage>,
        ws_receiver: SplitStream<WebSocketStream<TcpStream>>
    ) -> Self 
    {
        Receiver { connection_handle, rx, ws_receiver }
    }
    
    pub async fn run(self) {
        
    }
}