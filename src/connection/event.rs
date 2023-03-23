use crate::parser::Parser;
use crate::scheme::RequestHandle;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum ConnectionEvent<P: Parser> {
    Close,
    EventMessage(P::TheirEvent),
    RequestMessage(RequestHandle<P>),
}
