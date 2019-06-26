#![allow(unused)]

#[macro_use]
extern crate serde;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use mantle::{Address, Context, Event, Service};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InnerTy {
    Field1,
    Field2,
}

pub type Tuple = (Address, u64, Address);

#[derive(Serialize, Deserialize, Event, Default)]
pub struct TestEvent {
    #[indexed]
    indexed: DefTy,
    non_indexed: (u32, u32),
}

#[derive(Service)]
pub struct TestService {}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct DefTy {
    f1: Option<i64>,
    f2: Vec<Option<DefTy>>,
    f3: HashMap<String, InnerTy>,
    f4: Tuple,
}

#[derive(Serialize, Deserialize, Event, Default)]
pub struct TestEvent2 {
    #[indexed]
    indexed1: u32,
    #[indexed]
    indexed2: u32,
}

type Result<T> = std::result::Result<T, ()>;

impl TestService {
    pub fn new(ctx: &Context, name: String) -> Result<Self> {
        unimplemented!()
    }

    pub fn the(&self, ctx: &Context, arg1: Vec<DefTy>, arg2: Vec<u8>) -> Result<HashSet<Address>> {
        unimplemented!()
    }

    pub fn it(
        &mut self,
        ctx: &Context,
        a1: BTreeMap<bool, [u32; 12]>,
        a3: BTreeSet<i64>,
    ) -> Result<()> {
        unimplemented!()
    }

    fn private(&self, ctx: &Context, arg: String) -> Result<u64> {
        TestEvent::default().emit();
        unimplemented!()
    }

    pub fn void(&self, ctx: &Context) -> Result<()> {
        let event = TestEvent2::default();
        let event_ref = &event;
        Event::emit(&*event_ref);
        unimplemented!()
    }

    pub fn import(&mut self, ctx: &Context, imported: testlib::RpcType) -> Result<(bool, char)> {
        Event::emit(&testlib::RandomEvent {
            the_topic: "hello".to_string(),
            the_data: "world".to_string(),
        });
        unimplemented!()
    }

    pub fn default(&mut self, ctx: &Context, nondefault_arg: u32) -> Result<()> {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(TestService);
}

#[test]
fn test_mantle_build() {
    let idl_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/service/TestService.json"
    ))
    .unwrap();

    let actual: serde_json::Value = serde_json::from_str(&idl_json).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/res/TestService.json"
    )))
    .unwrap();

    assert_eq!(actual, expected);
}
