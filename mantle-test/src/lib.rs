mod ext;

use std::cell::RefCell;
// use std::collections::HashMap;

use blockchain_traits::Blockchain as _;
use mantle_types::Address;
use memchain::Memchain;

const SEED_ADDR: Address = Address([0xffu8; 20]);

thread_local! {
    static MEMCHAIN: RefCell<Memchain<'static>> =
        RefCell::new(Memchain::new("testnet".to_string(), {
            let mut genesis_state = std::collections::HashMap::new();
            genesis_state.insert(SEED_ADDR, std::borrow::Cow::Owned(memchain::Account {
                balance: u64::max_value(),
                ..Default::default()
            }));
            genesis_state
        }));
    static NEXT_ADDR: RefCell<u64> = RefCell::new(0);
}

pub fn create_account(initial_balance: u64) -> Address {
    MEMCHAIN.with(|memchain| {
        let mut memchain = memchain.borrow_mut();

        let new_addr = NEXT_ADDR.with(|next_addr| {
            let mut next_addr = next_addr.borrow_mut();
            let mut addr = Address::default();
            loop {
                let next_addr_bytes = next_addr.to_le_bytes();
                (addr.0)[..next_addr_bytes.len()].copy_from_slice(&next_addr_bytes);
                *next_addr += 1;
                if memchain.metadata_at(&addr).is_none() {
                    break addr;
                }
            }
        });

        memchain.transact(
            SEED_ADDR,
            new_addr,
            initial_balance,
            Vec::new(), /* input */
            memchain::BASE_GAS,
            0, /* gas price */
        );

        new_addr
    })
}
