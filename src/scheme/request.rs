use serde::Serialize;

pub struct Request<T: Serialize> {
    pub to: String,
    pub data: T
}