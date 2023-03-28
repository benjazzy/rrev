use crate::parser::Parser;
use crate::server::multi_acceptor::MultiAcceptorHandle;
use crate::server::server_handle::AcceptorsServerHandle;
use crate::server::server_trait::Server;
use std::collections::HashMap;

pub struct Manager {
    acceptor_hdl: MultiAcceptorHandle,
}

impl Manager {
    pub fn new(server_names: Vec<String>) -> Self {
        todo!()
    }
}
