use std::fmt::Formatter;

#[derive(Debug)]
pub enum ListenAddrError {
    Io(tokio::io::Error),
    SendError,
    RecvError,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ClientsError {
    SendError,
    RecvError,
}

impl std::fmt::Display for ListenAddrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            ListenAddrError::Io(e) => "Io error getting listen address: {e}",
            ListenAddrError::SendError => "There was a problem sending the message.",
            ListenAddrError::RecvError => {
                "There was a problem receiving the address from the server."
            }
        };

        write!(f, "{message}")
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
