use crate::parser::Parser;
use crate::scheme::RequestHandle;
use std::net::SocketAddr;

/// Events that a Server can produce.
pub enum ServerEvent<P: Parser> {
    /// The server has closed.
    Close,

    /// There is a new connection to the server.
    /// Contains the address of the new connection.
    NewConnection(SocketAddr),

    /// A connection to the server closed
    /// Contains the address of the closed connection.
    ConnectionClose(SocketAddr),

    /// A connection sent a request.
    /// Contains the request and the address
    /// of the connection that sent the request.
    ConnectionRequest(ConnectionRequest<P>),

    /// A connection sent an event.
    /// Contains the event and the address
    /// of the connection that sent the event
    ConnectionEvent(ConnectionEvent<P::TheirEvent>),
}

/// Used in [ServerEvent]
/// Contains a request and the address
/// of the connection where the request came from.
pub struct ConnectionRequest<P: Parser> {
    /// Address of the connection that sent the request.
    pub from: SocketAddr,

    /// RequestHandle containing the request.
    pub request: RequestHandle<P>,
}

/// Used in [ServerEvent]
/// Contains a event and the address
/// of the connection where the event came from.
pub struct ConnectionEvent<T> {
    /// Address of the connection that sent the event.
    pub from: SocketAddr,

    /// Event
    pub event: T,
}
