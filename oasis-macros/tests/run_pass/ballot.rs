oasis_std::contract! {

#[macro_use]
extern crate failure;

#[derive(Contract)]
pub struct Ballot {
    owner: Address,
    options: Vec<String>,
    votes: Vec<(Address, usize)>,
}

impl Ballot {
    pub fn get_options(&self, _ctx: &Context) -> Result<&Vec<String>> {
        Ok(&self.options)
    }

    pub fn vote(&mut self, ctx: &Context, option_index: usize) -> Result<()> {
        let sender = ctx.sender();
        if option_index >= self.options.len() {
            return Err(format_err!("Option index out of bounds."));
        }
        if self.votes.iter().any(|(address, _)| address == &sender) {
            return Err(format_err!("Already voted!"));
        }
        self.votes.push((sender, option_index));
        Ok(())
    }

    pub fn tally_results(&self, ctx: &Context) -> Result<Vec<u32>> {
        if ctx.sender() != self.owner {
            return Err(format_err!("Permission denied."));
        }
        let mut results = vec![0; self.options.len()];
        for (_, option) in self.votes.iter() {
            results[*option] += 1;
        }
        Ok(results)
    }
}

impl Ballot {
    pub fn new(ctx: &Context, options: Vec<String>) -> Result<Self> {
        Ok(Self {
            owner: ctx.sender(),
            options,
            votes: Vec::new(),
        })
    }
}

}

fn main() {}
