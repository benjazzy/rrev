use tokio::sync::mpsc;
use crate::parser::Parser;

use crate::scheme::internal;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InternalMessage<P: Parser> {
    Close,
    NewMessage(internal::Message<P::TheirRequest, P::TheirReply, P::TheirEvent>),
}

#[derive(Clone)]
pub struct InternalHdl<P: Parser> {
    tx: mpsc::Sender<InternalMessage<P>>,
}

impl<P: Parser> InternalHdl<P> {
    pub fn new(tx: mpsc::Sender<InternalMessage<P>>) -> Self {
        InternalHdl { tx }
    }

    pub async fn close(&self) {
        let _ = self.tx.send(InternalMessage::Close).await;
    }

    pub async fn new_message(&self, message: internal::Message<P::TheirRequest, P::TheirReply, P::TheirEvent>) {
        let _ = self.tx.send(InternalMessage::NewMessage(message)).await;
    }
}
