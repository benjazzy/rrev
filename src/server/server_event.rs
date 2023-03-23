use crate::parser::Parser;
use crate::scheme::RequestHandle;
use std::net::SocketAddr;

pub enum ServerEvent<P: Parser> {
    Close,
    NewConnection(SocketAddr),
    ConnectionClose(SocketAddr),
    ConnectionRequest(ConnectionRequest<RequestHandle<P>>),
    ConnectionEvent(ConnectionEvent<P::TheirEvent>),
}

pub struct ConnectionRequest<T> {
    pub from: SocketAddr,
    pub request: T,
}

pub struct ConnectionEvent<T> {
    pub from: SocketAddr,
    pub event: T,
}
