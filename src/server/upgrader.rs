use crate::connection;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub async fn accept(stream: TcpStream) -> Option<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    todo!()
}
