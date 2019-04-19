#![allow(unused)]

#[macro_use]
extern crate serde;

#[derive(Serialize, Deserialize, Clone)]
pub struct RpcType {
    pub value: u32,
}

pub struct PrivateField {
    pub value: u32,
    pub(crate) private_field: u32,
}

pub struct NonPod {
    pub value: std::boxed::Box<NonPod>,
}
