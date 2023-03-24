/// Contains types of errors that can by produced in the server module.

use std::fmt::Formatter;

/// Con be produced when getting the listen address.
#[derive(Debug)]
pub enum ListenAddrError {
    /// Represents an io error.
    Io(tokio::io::Error),

    /// Problem sending the request.
    SendError,

    /// Problem receiving the reply.
    RecvError,
}

/// Can be produced when getting the connected clents from the server.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ClientsError {
    /// There was a problem sending the request to the server.
    SendError,

    /// There was a problem receiving the list of clients from the server.
    RecvError,
}

impl std::fmt::Display for ListenAddrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ListenAddrError::Io(e) => write!(f, "Io error getting listen address: {e}"),
            ListenAddrError::SendError => write!(f, "There was a problem sending the message."),
            ListenAddrError::RecvError => {
                write!(
                    f,
                    "There was a problem receiving the address from the server."
                )
            }
        }
    }
}

impl std::fmt::Display for ClientsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            ClientsError::SendError => "Problem sending get clients message to the server.",
            ClientsError::RecvError => "Problem receiving the reply from the server.",
        };

        write!(f, "{message}")
    }
}

impl std::error::Error for ListenAddrError {}
impl std::error::Error for ClientsError {}
