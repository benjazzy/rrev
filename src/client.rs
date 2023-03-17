use tokio_tungstenite::tungstenite;
use url::Url;
use crate::connection::ConnectionHdl;

pub async fn connect<
    OurReq,
    OurRep,
    OurEvent,
    TheirReq,
    TheirRep,
    TheirEvent
>(url: Url) -> Result<ConnectionHdl<
    OurReq,
    OurRep,
    OurEvent,
    TheirReq,
    TheirRep,
    TheirEvent
>, tungstenite::error::Error> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;

    // let hdl = ConnectionHdl::new(ws_stream);
    todo!()
}