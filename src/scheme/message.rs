use serde::{ Serialize, Deserialize };

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Message<Req, Rep, Event> {
    Request{ id: usize, data:Req },
    Reply{ id: usize, data:Rep },
    Event(Event),
}