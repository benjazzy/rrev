#![feature(assert_matches)]
#![feature(async_closure)]

extern crate core;

pub mod client;
mod connection;
pub mod error;
pub mod parser;
mod scheme;
mod sender_manager;
pub mod server;

pub use scheme::Requestable;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
