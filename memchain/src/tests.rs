#![cfg(test)]

use crate::*;

const ETH: u64 = 1_000_000_000;

fn eth(num: u64) -> U256 {
    U256::from(num * ETH)
}

extern "C" fn nop_main(_bc: *mut dyn BlockchainIntrinsics) -> u16 {
    0
}

extern "C" fn simple_main(bc: *mut dyn BlockchainIntrinsics) -> u16 {
    let mut bc = unsafe { &mut *bc };
    let mut input = bc.fetch_input();
    input.push(4);
    bc.ret(input);
    0
}

extern "C" fn fail_main(bc: *mut dyn BlockchainIntrinsics) -> u16 {
    let mut bc = unsafe { &mut *bc };
    bc.err(r"¯\_(ツ)_/¯".as_bytes().to_vec());
    1
}

extern "C" fn invoke_subtx(bc: *mut dyn BlockchainIntrinsics) -> u16 {
    let mut bc = unsafe { &mut *bc };
    bc.transact(
        Address::default(),
        Address::from(2),
        U256::from(3),
        vec![1u8, 2, 3],
        U256::from(1_000_000),
        U256::zero(),
    );
    0
}

fn create_bc<'bc>(
    mains: Vec<Option<extern "C" fn(*mut dyn BlockchainIntrinsics) -> u16>>,
) -> Rc<RefCell<Blockchain<'bc>>> {
    let genesis_state = mains
        .into_iter()
        .enumerate()
        .map(|(i, main)| {
            (
                Address::from(i + 1),
                Cow::Owned(Account {
                    balance: U256::from((i as u64 + 1) * ETH),
                    code: Vec::new(),
                    storage: {
                        let mut storage = HashMap::new();
                        storage.insert(
                            "common_key".to_string().into_bytes(),
                            "common_value".to_string().into_bytes(),
                        );
                        storage.insert(
                            format!("key_{}", i).into_bytes(),
                            format!("value_{}", i).into_bytes(),
                        );
                        storage
                    },
                    expiry: None,
                    main,
                }),
            )
        })
        .collect();

    Blockchain::new(genesis_state)
}

#[test]
fn transfer() {
    let bc = create_bc(vec![None, Some(nop_main)]);
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        eth(1)
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        eth(2)
    );
    let value = U256::from(50);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        value,
        Vec::new(),
        U256::from(BASE_GAS),
        U256::from(1),
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        U256::from(eth(1) - U256::from(BASE_GAS) - value),
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        U256::from(eth(2) + value),
    );
}

#[test]
fn simple_tx() {
    let bc = create_bc(vec![None, Some(simple_main)]);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        U256::from(50),
        vec![1, 2, 3],
        U256::from(BASE_GAS),
        U256::zero(),
    );
    assert_eq!(bc.borrow().fetch_ret(), vec![1, 2, 3, 4]);
}

#[test]
fn revert_tx() {
    let bc = create_bc(vec![None, Some(fail_main)]);
    bc.borrow_mut().transact(
        Address::from(1),
        Address::from(2),
        U256::from(10000),
        Vec::new(),
        U256::from(BASE_GAS),
        U256::from(1),
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(1)).unwrap().balance,
        U256::from(eth(1) - U256::from(BASE_GAS)),
    );
    assert_eq!(
        bc.borrow().metadata_at(&Address::from(2)).unwrap().balance,
        U256::from(eth(2)),
    );
}

#[test]
fn subtx() {}
