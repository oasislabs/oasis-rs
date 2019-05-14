#![allow(unused)]
extern crate bcfs;

use std::{cell::RefCell, rc::Rc};

struct Dummy;

impl bcfs::KVStore for Dummy {
    fn contains(&self, key: &[u8]) -> bool {
        false
    }

    fn size(&self, key: &[u8]) -> u64 {
        0
    }

    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        None
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {}
}

impl bcfs::BlockchainIntrinsics for Dummy {
    fn input(&self) -> Vec<u8> {
        Vec::new()
    }
    fn input_len(&self) -> u64 {
        0
    }

    fn ret(&mut self, data: Vec<u8>) {}

    fn ret_err(&mut self, data: Vec<u8>) {}

    fn emit(&mut self, topics: Vec<Vec<u8>>, data: Vec<u8>) {}

    fn code_at(&self, addr: &str) -> Option<Vec<u8>> {
        None
    }
    fn code_len(&self, addr: &str) -> u64 {
        0
    }

    fn metadata_at(&self, addr: &str) -> Option<bcfs::AccountMetadata> {
        None
    }
}

fn main() {
    let dummy = Rc::new(RefCell::new(Dummy));
    let dummy_kv: Rc<RefCell<dyn bcfs::KVStore>> = Rc::new(RefCell::new(Dummy));
    let dummy_bci: Rc<RefCell<dyn bcfs::BlockchainIntrinsics>> = Rc::new(RefCell::new(Dummy));
    let mut bcfs = bcfs::BCFS::new(dummy_kv, dummy_bci, "00000000000000000000".to_string());
    bcfs.seek(3u32.into(), 0, wasi_types::Whence::Start)
        .unwrap();
    println!("Hello, world!");
}
