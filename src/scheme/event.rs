use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Event<T> {
    from: String,
    data: T,
}