#![allow(dead_code)]

use oasis_std::{abi::EndpointInterface, derive::eth_abi};

#[eth_abi(DoubleArrayEndpoint, DoubleArrayClient)]
pub trait DoubleArrayContract {
    fn double_array(&mut self, v: [u8; 16]);
}

const PAYLOAD_SAMPLE_1: &[u8] = &[
    0x71, 0x3d, 0x4b, 0x80, 0x12, 0x24, 0x36, 0x48, 0x60, 0x72, 0x84, 0x96, 0x07, 0x14, 0x21, 0x28,
    0x35, 0x42, 0x49, 0x56, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

#[test]
fn bytes16() {
    #[derive(Default)]
    pub struct Instance {
        pub v1: [u8; 8],
        pub v2: [u8; 8],
    }

    impl DoubleArrayContract for Instance {
        fn double_array(&mut self, v: [u8; 16]) {
            self.v1.copy_from_slice(&v[0..8]);
            self.v2.copy_from_slice(&v[8..16]);
        }
    }

    let mut endpoint = DoubleArrayEndpoint::new(Instance::default());

    endpoint.dispatch(PAYLOAD_SAMPLE_1);

    assert_eq!(
        endpoint.inner.v1,
        [0x12, 0x24, 0x36, 0x48, 0x60, 0x72, 0x84, 0x96]
    );
    assert_eq!(
        endpoint.inner.v2,
        [0x07, 0x14, 0x21, 0x28, 0x35, 0x42, 0x49, 0x56]
    );
}
