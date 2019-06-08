use mantle::{Address, Context};

#[derive(mantle::Service)]
pub struct Ballot {
    owner: Address,
    options: Vec<String>,
    votes: Vec<(Address, usize)>,
}

impl Ballot {
    pub fn new(ctx: &Context, options: Vec<String>) -> Result<Self, ()> {
        Ok(Self {
            owner: ctx.sender(),
            options,
            votes: Vec::new(),
        })
    }
}

impl Ballot {
    pub fn get_options(&self, _ctx: &Context) -> Result<&Vec<String>, String> {
        Ok(&self.options)
    }

    pub fn vote(&mut self, ctx: &Context, option_index: usize) -> Result<(), String> {
        let sender = ctx.sender();
        if option_index >= self.options.len() {
            return Err(format!("Option index out of bounds."));
        }
        if self.votes.iter().any(|(address, _)| address == &sender) {
            return Err(format!("Already voted!"));
        }
        self.votes.push((sender, option_index));
        Ok(())
    }

    pub fn tally_results(&self, ctx: &Context) -> Result<Vec<u32>, String> {
        if ctx.sender() != self.owner {
            return Err(format!("Permission denied."));
        }
        let mut results = vec![0; self.options.len()];
        for (_, option) in self.votes.iter() {
            results[*option] += 1;
        }
        Ok(results)
    }
}

fn main() {
    mantle::service!(Ballot);
}
