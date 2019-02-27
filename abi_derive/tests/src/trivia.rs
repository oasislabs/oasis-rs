#![allow(dead_code)]
// Minimal interface test/example

use owasm_abi_derive::eth_abi;

#[eth_abi(Endpoint, Client)]
pub trait Contract {
    fn constructor(&mut self, _p: bool);
    fn sam(&mut self, _p1: Vec<u8>) -> u32;

    #[event]
    fn baz_fired(&mut self, indexed_p1: u32, p2: u32);
}
