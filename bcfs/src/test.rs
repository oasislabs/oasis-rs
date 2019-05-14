#![cfg(test)]

use super::*;

use std::collections::HashMap;


#[test]
fn test() {
    let kvs = Rc::new(RefCell::new(InMemoryKVStore::new()));
    let bci = Rc::new(RefCell::new(InMemoryBlockchain::new()))
    let mut vfs = BCFS::new(kvs, bci, "00000000000000000000".to_string());
}
