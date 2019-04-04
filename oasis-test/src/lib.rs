mod ext;

use oasis_std::types::*;

pub fn create_account<V: Into<U256>>(endowment: V) -> Address {
    let mut addr_bytes = [0u8; 20];
    let mut endowment_bytes = [0u8; 32];
    endowment.into().to_big_endian(&mut endowment_bytes);
    ext::create(
        endowment_bytes.as_ptr(),
        std::ptr::null(),
        0,
        addr_bytes.as_mut_ptr(),
    );
    Address::from(addr_bytes)
}
