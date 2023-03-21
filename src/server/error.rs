use std::fmt::{Display, Formatter};

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

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "Io Error: {e}"),
            Error::RecvError => write!(f, "There was a problem receiving."),
            Error::SendError => write!(f, "There was a problem sending.",),
        }
    }
}
