use crate::connection::ConnectionEvent;
use crate::error::SendError;
use crate::error::{RequestError, TimeoutError};
use crate::parser::Parser;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
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

    pub async fn close(&self) -> Result<(), SendError> {
        self.tx
            .send(ConnectionMessage::Close)
            .await
            .map_err(|_| SendError)
    }

    pub async fn request_with_sender(
        &self,
        request: P::OurRequest,
        tx: oneshot::Sender<Result<P::TheirReply, RequestError>>,
    ) -> Result<(), RequestError> {
        self.tx
            .send(ConnectionMessage::Request { data: request, tx })
            .await
            .map_err(|_| RequestError::SendError)
    }

    pub async fn request(&self, request: P::OurRequest) -> Result<P::TheirReply, RequestError> {
        let (tx, rx) = oneshot::channel();
        self.request_with_sender(request, tx).await?;

        match rx.await {
            Ok(r) => r,
            Err(_) => Err(RequestError::RecvError),
        }
    }

    pub async fn request_timeout(
        &self,
        request: P::OurRequest,
        timeout: Duration,
    ) -> Result<P::TheirReply, TimeoutError> {
        match tokio::time::timeout(timeout, self.request(request)).await {
            Ok(result) => result.map_err(TimeoutError::RequestError),
            Err(_) => Err(TimeoutError::Timeout),
        }
    }

    pub async fn event(&self, event: P::OurEvent) -> Result<(), SendError> {
        self.tx
            .send(ConnectionMessage::Event(event))
            .await
            .map_err(|_| SendError)
    }
}
