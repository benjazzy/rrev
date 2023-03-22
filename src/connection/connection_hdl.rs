use crate::connection::ConnectionEvent;
use crate::parser::Parser;
use crate::request_error::RequestError;
use crate::scheme::RequestHandle;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::{Connection, ConnectionMessage};

#[derive(Debug, Clone)]
pub struct ConnectionHdl<P: Parser> {
    tx: mpsc::Sender<ConnectionMessage<P>>,
}

impl<P: Parser> ConnectionHdl<P> {
    pub async fn new(
        stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        event_tx: mpsc::Sender<ConnectionEvent<P>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1);

        let connection: Connection<P> = Connection::new(rx, stream, event_tx);

        tokio::spawn(connection.run());

        ConnectionHdl { tx }
    }

    pub async fn close(&self) {
        self.tx.send(ConnectionMessage::Close).await;
    }

    pub async fn request_with_sender(
        &self,
        request: P::OurRequest,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
    ) {
        let _ = self
            .tx
            .send(ConnectionMessage::Request { data: request, tx })
            .await;
    }

    pub async fn request(&self, request: P::OurRequest) -> Result<P::TheirReply, RequestError> {
        let (tx, rx) = oneshot::channel();
        self.request_with_sender(request, tx).await;

        match rx.await {
            Ok(r) => r,
            Err(e) => Err(RequestError::Recv(e)),
        }
    }

    pub async fn request_timeout(
        &self,
        request: P::OurRequest,
        timeout: Duration,
    ) -> Result<P::TheirReply, RequestError> {
        match tokio::time::timeout(timeout, self.request(request)).await {
            Ok(result) => result,
            Err(_) => return Err(RequestError::Timeout),
        }
    }

    pub async fn event(&self, event: P::OurEvent) {
        let _ = self.tx.send(ConnectionMessage::Event(event)).await;
    }
}
