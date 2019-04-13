#![feature(proc_macro_hygiene)]

#[oasis_std::contract]
mod contract {
    #[derive(Contract)]
    pub struct EthabiTest;

    impl EthabiTest {
        pub fn new(_ctx: &Context) -> Result<Self> {
            Ok(Self {})
        }

        pub fn func(
            &self,
            _ctx: &Context,
            _inp: usize,
            _more_inps: (u8, [U256; 4], [[bool; 10]; 2], (i32, i64, (u128))),
        ) -> Result<((H160, Address, H256, U256), Vec<u8>, Vec<Vec<&str>>)> {
            unimplemented!()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn test_generated_abi() {
        let expected_abi_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/res/EthabiTest.json"))
                .unwrap(),
        )
        .unwrap();
        let expected_fns: HashMap<String, &serde_json::Value> = expected_abi_json
            .as_array()
            .unwrap()
            .iter()
            .map(|def| {
                // snake_case is the correct case
                let name = if def["type"].as_str().unwrap() == "constructor" {
                    "constructor".to_string()
                } else {
                    def["name"].as_str().unwrap().to_string()
                };
                (name, def)
            })
            .collect();

        let abi_json: serde_json::Value =
            serde_json::from_str(include_str!(concat!(env!("ABI_DIR"), "/EthabiTest.json")))
                .unwrap();
        let abi_fns = abi_json.as_array().unwrap();

        assert_eq!(expected_fns.len(), abi_fns.len());

        for def in abi_fns.iter() {
            let expected = expected_fns
                .get(if def["type"].as_str().unwrap() == "constructor" {
                    "constructor"
                } else {
                    def["name"].as_str().unwrap()
                })
                .unwrap();

            assert_eq!(expected, &def);
        }
    }
}
