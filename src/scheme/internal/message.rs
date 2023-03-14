use serde::{ Serialize, Deserialize };

use super::{ Request, Reply};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Message<Req, Rep, Event> {
    Request(Request<Req>),
    Reply(Reply<Rep>),
    Event(Event),
}