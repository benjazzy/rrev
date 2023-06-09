use crate::connection::SenderHdl;
use crate::error::SendError;
use crate::parser::Parser;
use crate::scheme::internal;

#[derive(Debug, Clone)]
pub struct RequestHandle<P: Parser> {
    request: internal::Request<P::TheirRequest>,
    sender_hdl: SenderHdl<P>,
}

impl<'a, P: Parser> RequestHandle<P> {
    pub fn new(request: internal::Request<P::TheirRequest>, sender_hdl: SenderHdl<P>) -> Self {
        RequestHandle {
            request,
            sender_hdl,
        }
    }

    pub fn get_request(&'a self) -> &'a P::TheirRequest {
        &self.request.data
    }

    pub async fn complete(&self, reply: P::OurReply) -> Result<(), SendError> {
        let message = internal::Message::Reply(internal::Reply {
            id: self.request.id,
            data: reply,
        });
        self.sender_hdl.send(message).await.map_err(|_| SendError)
    }
}
