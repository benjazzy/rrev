use std::fmt::Formatter;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SendError;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RequestError {
    SendError,
    RecvError,
    Canceled,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TimeoutError {
    RequestError(RequestError),
    Timeout,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "There was a problem sending the message.")
    }
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            RequestError::SendError => "There was a problem sending the request.",
            RequestError::RecvError => "There was a problem receiving the reply.",
            RequestError::Canceled => "The request was canceled before it could be completed.",
        };

        write!(f, "{message}")
    }
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeoutError::Timeout => write!(f, "Timeout before request could be completed."),
            TimeoutError::RequestError(r) => write!(f, "{r}"),
        }
    }
}

impl std::error::Error for SendError {}
impl std::error::Error for RequestError {}
impl std::error::Error for TimeoutError {}
