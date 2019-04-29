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

#[derive(Serialize, Deserialize, Clone, oasis_std::Event)]
pub struct RandomEvent {
    #[indexed]
    pub the_topic: String,
    pub the_data: String,
}
