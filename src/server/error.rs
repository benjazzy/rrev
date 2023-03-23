use std::fmt::{Display, Formatter};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Error {
    /// Used when getting the listen address of the Listener socket if
    /// the socket is not listening.
    Io(tokio::io::Error),

    /// Problem receiving the reply from Listener.
    RecvError,

    /// Problem sending message to the Listener.
    SendError,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RequestError {
    Timeout,
    Recv(oneshot::error::RecvError),
    Closed,
    ConnectionNotFound,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "Io Error: {e}"),
            Error::RecvError => write!(f, "There was a problem receiving."),
            Error::SendError => write!(f, "There was a problem sending.",),
        }
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
            RequestError::ConnectionNotFound => "The requested connection was not found.",
        };
        write!(f, "{message}")
    }
}

impl From<crate::connection::RequestError> for RequestError {
    fn from(value: crate::connection::RequestError) -> Self {
        match value {
            crate::connection::RequestError::Timeout => RequestError::Timeout,
            crate::connection::RequestError::Recv(e) => RequestError::Recv(e),
            crate::connection::RequestError::Closed => RequestError::Closed,
        }
    }
}
