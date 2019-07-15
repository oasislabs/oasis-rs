#![allow(unused)]

#[macro_use]
extern crate serde;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use mantle::{Address, Context, Event, Service};
use map_vec::{Map, Set};

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

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TupleStruct(pub String, pub String, pub i32);

#[derive(Serialize, Deserialize, Event, Default)]
pub struct TestEvent2 {
    #[indexed]
    indexed1: u32,
    #[indexed]
    indexed2: u32,
}

type Result<T> = std::result::Result<T, ()>;

impl TestService {
    pub fn new(ctx: &Context, tuple_struct: TupleStruct) -> Self {
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
    ) -> std::result::Result<Vec<u8>, Map<String, String>> {
        unimplemented!()
    }

    fn private(&self, ctx: &Context, arg: String) -> u64 {
        TestEvent::default().emit();
        unimplemented!()
    }

    pub fn void(&self, ctx: &Context) {
        let event = TestEvent2::default();
        let event_ref = &event;
        Event::emit(&*event_ref);
        unimplemented!()
    }

    #[mantle::default]
    pub fn the_default_fn(&mut self, ctx: &Context) -> std::result::Result<Option<u64>, Set<u32>> {
        unimplemented!()
    }
}

fn main() {
    mantle::service!(TestService);
}
