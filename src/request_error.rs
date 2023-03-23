use std::fmt::{Display, Formatter};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RequestError {
    Timeout,
    Recv(oneshot::error::RecvError),
    Closed,
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
