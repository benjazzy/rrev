#![feature(assert_matches)]
#![feature(async_closure)]

extern crate core;

pub mod client;
mod connection;
pub mod error;
pub mod parser;
mod scheme;
pub mod server;

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
