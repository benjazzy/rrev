use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum RouteReply {
    Ok,
    UnknownRoute
}