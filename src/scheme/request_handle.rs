use futures_util::{FutureExt, TryFutureExt};
use serde::Serialize;
use tokio::sync::oneshot;

use crate::connection::SenderHdl;
use crate::scheme::internal;

#[derive(Debug, Clone)]
pub struct RequestHandle<OurReq: Serialize, OurRep: Serialize, OurEvent: Serialize, TheirReq> {
    request: internal::Request<TheirReq>,
    sender_hdl: SenderHdl<OurReq, OurRep, OurEvent>,
}

impl<
        'a,
        OurReq: Serialize + Send + 'static,
        OurRep: Serialize + Send + 'static,
        OurEvent: Serialize + Send + 'static,
        TheirReq,
    > RequestHandle<OurReq, OurRep, OurEvent, TheirReq>
{
    pub fn new(
        request: internal::Request<TheirReq>,
        sender_hdl: SenderHdl<OurReq, OurRep, OurEvent>,
    ) -> Self {
        RequestHandle {
            request,
            sender_hdl,
        }
    }

    pub fn get_request(&'a self) -> &'a TheirReq {
        &self.request.data
    }

    pub async fn complete(&self, reply: OurRep) {
        let message = internal::Message::Reply(internal::Reply {
            id: self.request.id,
            data: reply,
        });
        self.sender_hdl.send(message).await
    }
}
