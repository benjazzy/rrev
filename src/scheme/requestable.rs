use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[async_trait::async_trait]
pub trait Requestable: Sized {
    type Req: Serialize;
    type Rep: for<'a> Deserialize<'a>;

    fn new(to: String, request: Self::Req) -> (Self, oneshot::Receiver<Self::Rep>);
    fn get_request(&self) -> &Self::Req;
    async fn complete(self, reply: Self::Rep);
}
