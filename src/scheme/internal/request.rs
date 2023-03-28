use serde::{Deserialize, Serialize};

mod route_request;

pub use route_request::RouteRequest;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum RequestType<T> {
    Route(RouteRequest),
    User(T),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Request<T> {
    pub id: usize,
    pub data: RequestType<T>,
}
