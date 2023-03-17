use serde::{Deserialize, Serialize};

use super::{Reply, Request};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Message<Req, Rep, Event> {
    Request(Request<Req>),
    Reply(Reply<Rep>),
    Event(Event),
}
