use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Parser should be implemented by the user of this library.
/// Parser contains the types of messages that can be sent to and from a websocket.
///
/// # Examples
/// See [StringParser]
pub trait Parser: Clone + Debug + 'static {
    /// OurRequest is a request that we can send.
    /// The other connection should send [Self::TheirReply] in response
    type OurRequest: Serialize + Send + Debug + 'static;

    /// OurReply is a reply that the user should
    /// send in response to [Self::TheirRequest].
    type OurReply: Serialize + Send + Debug + 'static;

    /// OurEvent is a event that we can send.
    type OurEvent: Serialize + Send + Debug + 'static;

    /// TheirRequest is a request that we can receive.
    /// The user should send [Self::TheirReply] in response.
    type TheirRequest: for<'a> Deserialize<'a> + Clone + Send + Debug + 'static;

    /// TheirReply is a reply that we can receive
    /// in response to [Self::OurRequest].
    type TheirReply: for<'a> Deserialize<'a> + Clone + Send + Debug + 'static;

    /// TheirEvent is a event that we can receive.
    type TheirEvent: for<'a> Deserialize<'a> + Clone + Send + Debug + 'static;
}

/// Used for testing.
/// All types are String.
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
