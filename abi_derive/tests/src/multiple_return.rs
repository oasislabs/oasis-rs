#![allow(dead_code)]

use oasis_std::{abi::EndpointInterface, derive::eth_abi};

#[eth_abi(TupleReturnEndpoint, TupleReturnClient)]
pub trait TupleReturnContract {
    fn ret2(&mut self) -> (u64, u64);
    fn ret6(&mut self) -> (u64, u64, u64, u64, u64, u64);
    fn ret_var(&mut self) -> (u64, Vec<u8>);
}

#[test]
fn multiple_return() {
    pub struct Instance;

    impl TupleReturnContract for Instance {
        fn ret2(&mut self) -> (u64, u64) {
            (2, 2)
        }
        fn ret6(&mut self) -> (u64, u64, u64, u64, u64, u64) {
            (6, 6, 6, 6, 6, 6)
        }
        fn ret_var(&mut self) -> (u64, Vec<u8>) {
            (6, vec![1, 2, 3, 5, 7, 11])
        }
    }

    let mut endpoint = TupleReturnEndpoint::new(Instance);

    let res2 = endpoint.dispatch(&[0xa6, 0x37, 0xe6, 0x9c]);
    assert_eq!(
        &res2[..],
        &[
            0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 2
        ][..]
    );

    let res6 = endpoint.dispatch(&[0x8f, 0x34, 0x96, 0x73]);
    assert_eq!(
        &res6[..],
        &[
            0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6
        ][..]
    );

    let res_var = endpoint.dispatch(&[0x50, 0x47, 0xe7, 0x52]);
    assert_eq!(
        &res_var[..],
        &[
            0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 1, 2, 3, 5, 7, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ][..]
    );
}
