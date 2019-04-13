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
    use super::*;

    #[test]
    fn test_xcc() {
        // 1. create user with initial `val`
        // 2. transfer all value to `ContractA`
        // 3. create `ContractB` which records the amount of value passed through it
        // 4. transfer `val - 1` to `ContractB`
        // 5. transfer `1` to `ContractB`

        let val = U256::from(0x0A515);

        let user = oasis_test::create_account(val);
        let ctx = Context::default().with_sender(user);

        let b = xcc_b::ContractB::new(&ctx).unwrap();
        let a = ContractA::new(&ctx.with_value(val), b.address()).unwrap();

        assert_eq!(a.do_the_thing(&ctx.with_value(val - 1)).unwrap(), val - 1);
        assert_eq!(b.total_value(&ctx).unwrap(), val - 1);

        assert_eq!(a.do_the_thing(&ctx.with_value(1)).unwrap(), 1u32);
        assert_eq!(b.total_value(&ctx).unwrap(), val);
    }
}
