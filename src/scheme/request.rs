use crate::Requestable;

pub struct Request<T: Requestable> {
    pub to: String,
    pub data: T,
}
