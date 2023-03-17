use crate::Requestable;
use serde::Serialize;

pub struct Request<T: Requestable> {
    pub to: String,
    pub data: T,
}
