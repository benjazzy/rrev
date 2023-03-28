use serde::{Deserialize, Serialize};

mod route_reply;

pub use route_reply::RouteReply;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ReplyType<T> {
    Route(RouteReply),
    User(T),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Reply<T> {
    pub id: usize,
    pub data: ReplyType<T>,
}
