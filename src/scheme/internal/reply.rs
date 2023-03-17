use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Reply<T> {
    pub id: usize,
    pub data: T,
}
