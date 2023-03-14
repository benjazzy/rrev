use serde::Serialize;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;

use super::{ Connection, ConnectionMessage };

#[derive(Clone)]
pub struct ConnectionHdl<T: Serialize> {
    tx: mpsc::Sender<ConnectionMessage<T>>
}

impl<T: Serialize + Send + 'static> ConnectionHdl<T> {
    pub fn new(stream: WebSocketStream<TcpStream>) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let connection = Connection::<T>::new(rx, stream);

        tokio::spawn(connection.run());

        ConnectionHdl { tx }
    }

    pub async fn close(&self) {
        let _ = self.tx.send(ConnectionMessage::Close).await;
    }
}
