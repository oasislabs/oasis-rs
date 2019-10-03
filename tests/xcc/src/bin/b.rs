use oasis_std::{Context, Service};
use serde::{Deserialize, Serialize};

#[derive(Service)]
pub struct ServiceB {
    seed: Number,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Number(pub u8);

#[derive(Serialize, Deserialize, Clone)]
pub struct RefWrapper<'a> {
    pub field: &'a str,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Greeting {
    Formal { title: String, name: String },
    Informal(String),
}

impl ServiceB {
    pub fn new(_ctx: &Context, seed: Number) -> Self {
        Self { seed }
    }

    pub fn say_hello(&self, _ctx: &Context, greeting: Greeting) -> String {
        match greeting {
            Greeting::Formal { title, name } => format!("hello {} {}", title, name),
            Greeting::Informal(name) => format!("yo {}. what up?", name),
        }
    }

    pub fn return_ref_struct<'a>(&self, _ctx: &Context, value: &'a str) -> RefWrapper<'a> {
        RefWrapper { field: value }
    }

    pub fn random(&self, _ctx: &Context, count: Number) -> Vec<Number> {
        vec![Number(4); count.0 as usize]
    }
}

fn main() {
    oasis_std::service!(ServiceB);
}
