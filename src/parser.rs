use serde::{Deserialize, Serialize};

/// Parser should be implemented by the user of this library.
/// Parser contains the types of messages that can be sent to and from a websocket.
///
/// # Examples
/// See [StringParser]
pub trait Parser: Clone + 'static {
    type OurRequest: Serialize + Send + 'static;
    type OurReply: Serialize + Send + 'static;
    type OurEvent: Serialize + Send + 'static;
    type TheirRequest: for<'a> Deserialize<'a> + Clone + Send + 'static;
    type TheirReply: for<'a> Deserialize<'a> + Clone + Send + 'static;
    type TheirEvent: for<'a> Deserialize<'a> + Clone + Send + 'static;

    fn our_request_to_string(request: &Self::OurRequest) -> serde_json::error::Result<String> {
        serde_json::to_string(request)
    }

    fn our_reply_to_string(reply: &Self::OurReply) -> serde_json::error::Result<String> {
        serde_json::to_string(reply)
    }

    fn our_event_to_string(event: &Self::OurEvent) -> serde_json::error::Result<String> {
        serde_json::to_string(event)
    }

    fn their_request_from_string(str: &str) -> serde_json::error::Result<Self::TheirRequest> {
        serde_json::from_str(str)
    }

    fn their_reply_from_string(str: &str) -> serde_json::error::Result<Self::TheirReply> {
        serde_json::from_str(str)
    }

    fn their_event_from_string(str: &str) -> serde_json::error::Result<Self::TheirEvent> {
        serde_json::from_str(str)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StringParser;

impl Parser for StringParser {
    type OurRequest = String;
    type OurReply = String;
    type OurEvent = String;
    type TheirRequest = String;
    type TheirReply = String;
    type TheirEvent = String;
}