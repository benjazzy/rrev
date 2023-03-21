use crate::scheme::RequestHandle;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use crate::parser::Parser;

use super::{Connection, ConnectionMessage};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RequestError {
    Timeout,
    Recv(oneshot::error::RecvError),
    Closed,
}

#[derive(Debug, Clone)]
pub struct ConnectionHdl<P: Parser> {
    tx: mpsc::Sender<ConnectionMessage<P>>,
}

impl<P: Parser> ConnectionHdl<P>
{
    pub async fn new(stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        let (tx, rx) = mpsc::channel(1);

        let connection: Connection<P> =
            Connection::new(rx, stream);

        tokio::spawn(connection.run());

        ConnectionHdl { tx }
    }

    pub async fn request(&self, request: P::OurRequest) -> Result<P::TheirReply, RequestError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(ConnectionMessage::Request { data: request, tx })
            .await;

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

    pub async fn register_request_listener(
        &self,
        tx: mpsc::Sender<RequestHandle<P>>,
    ) {
        self.tx.send(ConnectionMessage::RequestListener(tx)).await;
    }

    pub async fn register_event_listener(&self, tx: mpsc::Sender<P::TheirEvent>) {
        let _ = self.tx.send(ConnectionMessage::EventListener(tx)).await;
    }
}

impl Display for RequestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            RequestError::Timeout => "The request did not get a reply before the timeout.",
            RequestError::Recv(e) => "The request failed to get a reply {e}",
            RequestError::Closed => {
                "The connection was closed before the request could be completed."
            }
        };
        write!(f, "{message}")
    }
}
