use tokio::sync::oneshot;

use crate::connection::SenderHdl;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RequestHandle<Req, Rep, Event> {
    request: Req,
    sender_hdl: SenderHdl<Req, Rep, Event>,
}

impl<'a, Req, Rep, Event> RequestHandle<Req, Rep, Event> {
    pub fn new(request: Req, sender_hdl: SenderHdl<Req, Rep, Event>) -> Self {
        RequestHandle { request, sender_hdl }
    }

    pub fn get_request(&self) -> &'a Req {
        &self.request
    }

    pub fn complete(&self, reply: Rep) -> Result<(), Req> {
        self.tx.send(reply)
    }
}