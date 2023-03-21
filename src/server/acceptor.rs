use crate::connection;
use crate::parser::Parser;
use async_trait::async_trait;

#[async_trait]
pub trait Acceptor {
    type Parser: Parser;
    async fn accept(connection_hdl: connection::ConnectionHdl<Self::Parser>) -> bool;
}
