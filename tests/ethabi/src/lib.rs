#![feature(proc_macro_hygiene)]
///! Tests that generated ABI is compatible with `ballot.sol` from the Remix IDE.
use std::collections::HashMap;

#[oasis_std::contract]
mod contract {
    #[derive(Contract)]
    pub struct Ballot {
        chairperson: Address,
        voters: HashMap<Address, Voter>,
        votes: Vec<U256>,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub struct Voter {
        weight: U256,
        vote: Option<u8>,
        delegate: Option<Address>,
    }

    impl Ballot {
        /// Create a new ballot with `num_proposals` different proposals.
        pub fn new(ctx: &Context, num_proposals: u8) -> Result<Self> {
            let chairperson = ctx.sender();
            let mut voters = HashMap::new();
            voters.insert(
                chairperson,
                Voter {
                    weight: 1.into(),
                    ..Default::default()
                },
            );
            Ok(Self {
                chairperson,
                voters,
                votes: vec![U256::zero(); num_proposals as usize],
            })
        }

        /// Give `to_voter` the right to vote on this ballot. May only be called by `chairperson`.
        pub fn give_right_to_vote(&mut self, ctx: &Context, to_voter: Address) -> Result<()> {
            if ctx.sender() != self.chairperson {
                return Err(failure::format_err!("Permission denied."));
            }
            if let None = self.voters.get(&to_voter) {
                self.voters.insert(
                    to_voter,
                    Voter {
                        weight: 1.into(),
                        ..Default::default()
                    },
                );
            }
            Ok(())
        }

        /// Delegate your vote to the voter `to`.
        pub fn delegate(&mut self, ctx: &Context, to: Address) -> Result<()> {
            let sender = match self.voters.get(&ctx.sender()) {
                Some(sender) => sender,
                None => {
                    return Err(failure::format_err!(
                        "{:x} is not registered to vote",
                        ctx.sender()
                    ));
                }
            };
            if let Some(vote) = sender.vote {
                return Err(failure::format_err!(
                    "You already voted for proposal {}.",
                    vote
                ));
            }
            if let Some(delegate) = sender.delegate {
                return Err(failure::format_err!(
                    "You already delegated your vote to {}.",
                    delegate
                ));
            }

            let mut delegate_addr = to;
            loop {
                if let Some(next_delegate) =
                    self.voters.get(&delegate_addr).and_then(|d| d.delegate)
                {
                    if next_delegate == ctx.sender() {
                        return Err(failure::format_err!("Could not set up a delegate loop."));
                    }
                    delegate_addr = next_delegate;
                } else {
                    break;
                }
            }
            self.voters
                .get_mut(&ctx.sender())
                .unwrap()
                .delegate
                .replace(delegate_addr);
            let delegate = self.voters.entry(delegate_addr).or_default();
            if let Some(vote) = delegate.vote {
                self.votes[vote as usize] += 1.into();
            } else {
                delegate.weight += 1.into();
            }
            Ok(())
        }

        /// Give a single vote to proposal `to_proposal`.
        pub fn vote(&mut self, ctx: &Context, to_proposal: u8) -> Result<()> {
            let sender = match self.voters.get_mut(&ctx.sender()) {
                Some(sender) => sender,
                None => {
                    return Err(failure::format_err!("You are not registered to vote."));
                }
            };
            if let Some(vote) = sender.vote {
                return Err(failure::format_err!(
                    "You already voted for proposal {}.",
                    vote
                ));
            }
            sender.vote = Some(to_proposal);
            self.votes[to_proposal as usize] += 1.into();
            Ok(())
        }

        /// Returns the index of the current winning proposal.
        pub fn winning_proposal(&self, _ctx: &Context) -> Result<u8> {
            Ok(self
                .votes
                .iter()
                .enumerate()
                .max_by_key(|(_i, v)| *v)
                .unwrap()
                .0 as u8)
        }
    }

}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use heck::SnakeCase as _;

    #[test]
    fn test_generated_abi() {
        // `serde_json::Value` because `oasis_macros`, a proc_macro crate, can only export macros.
        // Besides, the ABI is an implementation detail.

        let mut expected_abi_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/res/Ballot.json"))
                .unwrap(),
        )
        .unwrap();
        let expected_fns: HashMap<String, &serde_json::Value> = expected_abi_json
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .map(|def| {
                // snake_case is the correct case
                let name = if def["type"].as_str().unwrap() == "constructor" {
                    "constructor".to_string()
                } else {
                    def["name"].as_str().unwrap().to_snake_case()
                };
                def["inputs"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .for_each(|inp| {
                        inp["name"] = inp["name"].as_str().unwrap().to_snake_case().into()
                    });
                (name, &*def)
            })
            .collect();

        let abi_json: serde_json::Value =
            serde_json::from_str(include_str!(concat!(env!("ABI_DIR"), "/Ballot.json"))).unwrap();
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

            assert_eq!(expected["type"], def["type"]);
            assert_eq!(expected["inputs"], def["inputs"]);

            match expected.get("outputs") {
                Some(expected_outputs) => {
                    let expected_outputs = expected_outputs.as_array().unwrap();
                    let outputs = def["outputs"].as_array().unwrap();
                    assert_eq!(expected_outputs.len(), outputs.len());
                    expected_outputs
                        .iter()
                        .zip(outputs.iter())
                        .for_each(|(eo, o)| {
                            assert_eq!(eo["type"], o["type"]) // Rust outputs don't have names
                        });
                }
                None => assert!(def.get("outputs").is_none()),
            }
        }
    }
}
