oasis_std::contract! {

#[derive(Contract)]
pub struct Ballot {
    owner: Address,
    options: Vec<String>,
    votes: Vec<(Address, usize)>,
}

impl Ballot {
    pub fn get_options(&self, _ctx: Context) -> &Vec<String> {
        &self.options
    }

    pub fn vote(&mut self, ctx: Context, option_index: usize) {
        let sender = ctx.sender();
        if !self.votes.iter().any(|(address, _)| address == &sender)
            && option_index < self.options.len()
        {
            self.votes.push((sender, option_index))
        }
    }

    pub fn tally_results(&self, ctx: Context) -> Vec<u32> {
        let mut results = vec![0; self.options.len()];
        if ctx.sender() == self.owner {
            for (_, option) in self.votes.iter() {
                results[*option] += 1;
            }
        }
        results
    }
}

impl Ballot {
    pub fn new(ctx: Context, options: Vec<String>) -> Self {
        Self {
            owner: ctx.sender(),
            options,
            votes: Vec::new(),
        }
    }
}

}

fn main() {}
