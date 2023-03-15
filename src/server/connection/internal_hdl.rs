use tokio::sync::mpsc;

use crate::scheme::internal;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InternalMessage<Req, Rep, Event> {
    Close,
    NewMessage(internal::Message<Req, Rep, Event>)
}

#[derive(Clone)]
pub struct InternalHdl<Req, Rep, Event> {
    tx: mpsc::Sender<InternalMessage<Req, Rep, Event>>,
}

impl<Req, Rep, Event> InternalHdl<Req, Rep, Event> {
    pub fn new(tx: mpsc::Sender<InternalMessage<Req, Rep, Event>>) -> Self {
        InternalHdl { tx }
    }
    
    pub async fn close(self) {
        let _ = self.tx.send(InternalMessage::Close).await;
    }

    pub async fn new_message(&self, message: internal::Message<Req, Rep, Event>) {
        let _ = self.tx.send(InternalMessage::NewMessage(message)).await;
    }
}