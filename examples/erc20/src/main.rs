#[macro_use]
extern crate serde;

use mantle::{Address, Context, Event, Service};

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, failure::Fail)]
pub enum Error {
    #[fail(display = "Unknown error occured.")]
    Unknown,

    #[fail(display = "Only existing admins can perform this operation.")]
    AdminPrivilegesRequired,

    #[fail(display = "Insuffient funds for transfer from {:?}.", address)]
    InsufficientFunds { address: Address },

    #[fail(display = "Address {:?} has no allowance from address {:?}.", from, to)]
    NoAllowanceGiven { from: Address, to: Address },

    #[fail(
        display = "Transfer request {} exceeds allowance {}.",
        amount, allowance
    )]
    RequestExceedsAllowance { amount: u64, allowance: u64 },
}

#[derive(Service, Default)]
pub struct ERC20Token {
    total_supply: u64,
    owner: Address,
    admins: HashSet<Address>,
    accounts: HashMap<Address, u64>,
    allowed: HashMap<Address, HashMap<Address, u64>>,
}

// A Transfer event struct
#[derive(Serialize, Deserialize, Clone, Debug, Default, Event)]
pub struct Transfer {
    #[indexed]
    pub from: Address,
    #[indexed]
    pub to: Address,
    #[indexed]
    pub amount: u64,
}

// An Approval event struct
#[derive(Serialize, Deserialize, Clone, Debug, Default, Event)]
pub struct Approval {
    #[indexed]
    pub sender: Address,
    #[indexed]
    pub spender: Address,
    #[indexed]
    pub amount: u64,
}

impl ERC20Token {
    /// Constructs a new `ERC20Token`
    pub fn new(ctx: &Context, total_supply: u64) -> Result<Self> {
        let owner = ctx.sender();
        let mut admins = HashSet::new();
        admins.insert(owner);
        let mut accounts = HashMap::new();
        accounts.insert(owner, total_supply);

        Ok(Self {
            total_supply,
            owner,
            admins,
            accounts,
            ..Default::default()
        })
    }

    /// Get balance
    pub fn balance_of(&mut self, ctx: &Context) -> Result<u64> {
        Ok(self
            .accounts
            .get(&ctx.sender())
            .copied()
            .unwrap_or_default())
    }

    /// Get total supply
    pub fn total_supply(&mut self, _ctx: &Context) -> Result<u64> {
        Ok(self.total_supply)
    }

    /// Add admin
    pub fn add_admin(&mut self, ctx: &Context, admin: Address) -> Result<()> {
        if !self.admins.contains(&ctx.sender()) {
            return Err(Error::AdminPrivilegesRequired);
        }
        self.admins.insert(admin);
        Ok(())
    }
}

// Helper methods

/// transfer method
fn do_transfer(
    accounts: &mut HashMap<Address, u64>,
    from: Address,
    to: Address,
    amount: u64,
) -> bool {
    let from_balance = accounts.get(&from).copied().unwrap_or_default();
    let to_balance = accounts.get(&to).copied().unwrap_or_default();

    // check for sufficient balance
    if from_balance < amount {
        return false;
    }
    accounts.insert(from, from_balance - amount);
    accounts.insert(to, to_balance + amount);

    Event::emit(&Transfer { from, to, amount });

    true
}

impl ERC20Token {
    /// transfer
    pub fn transfer(&mut self, ctx: &Context, to: Address, amount: u64) -> Result<Transfer> {
        let from = ctx.sender();
        if from == to || amount == 0u64 {
            // no-op
            return Ok(Transfer::default());
        }
        if do_transfer(&mut self.accounts, ctx.sender(), to, amount) {
            return Ok(Transfer { from, to, amount });
        }
        Err(Error::InsufficientFunds { address: from })
    }

    /// allowance
    pub fn approve(&mut self, ctx: &Context, spender: Address, amount: u64) -> Result<Approval> {
        let allowances = match self.allowed.entry(ctx.sender()) {
            Entry::Vacant(ve) => ve.insert(HashMap::new()),
            Entry::Occupied(oe) => oe.into_mut(),
        };
        allowances.insert(spender, amount);

        let approval = Approval {
            sender: ctx.sender(),
            spender,
            amount,
        };

        Event::emit(&approval);

        Ok(approval)
    }

    /// read allowance
    pub fn allowance(&mut self, ctx: &Context, spender: Address) -> Result<u64> {
        if !self.allowed.contains_key(&ctx.sender()) {
            return Ok(0u64);
        }
        Ok(self
            .allowed
            .get(&ctx.sender())
            .and_then(|allowances| allowances.get(&spender))
            .copied()
            .unwrap_or_default())
    }

    /// transfer from a given account up to the given allowance
    pub fn transfer_from(
        &mut self,
        _ctx: &Context,
        from: Address,
        spender: Address,
        amount: u64,
    ) -> Result<Transfer> {
        let allowances = self.allowed.get_mut(&from).unwrap();
        // if the spender is not in the list of addresses that are approved for automatic
        // withdrawal by the from address, then nothing can be done
        if !allowances.contains_key(&spender) {
            return Err(Error::NoAllowanceGiven { from, to: spender });
        }
        let allowance = allowances.get(&spender).copied().unwrap_or_default();
        // err if request is higher than allowance
        if allowance < amount {
            return Err(Error::RequestExceedsAllowance { amount, allowance });
        }
        if do_transfer(&mut self.accounts, from, spender, amount) {
            allowances.insert(spender, allowance - amount);
            return Ok(Transfer {
                from,
                to: spender,
                amount,
            });
        }
        Err(Error::InsufficientFunds { address: from })
    }
}

impl ERC20Token {
    /// mint new tokens
    pub fn mint(&mut self, ctx: &Context, amount: u64) -> Result<()> {
        if !self.admins.contains(&ctx.sender()) {
            return Err(Error::AdminPrivilegesRequired);
        }
        self.total_supply += amount;
        Ok(())
    }

    /// burn tokens from a given account
    pub fn burn(&mut self, ctx: &Context, from: Address, amount: u64) -> Result<()> {
        if !self.admins.contains(&ctx.sender()) {
            return Err(Error::AdminPrivilegesRequired);
        }
        let balance = self.accounts.get(&from).copied().unwrap_or_default();
        self.accounts
            .insert(from, std::cmp::max(0, balance - amount));
        Ok(())
    }
}

fn main() {
    mantle::service!(ERC20Token);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mantle::{Address, Context};

    /// Creates a new account and a `Context` with the new account as the sender.
    fn create_account() -> (Address, Context) {
        let addr = mantle_test::create_account(0 /* initial balance */);
        let ctx = Context::default().with_sender(addr).with_gas(100_000);
        (addr, ctx)
    }

    #[test]
    fn happy_paths() {
        let (_getafix, gctx) = create_account();
        let (_fulliautomatix, _fctx) = create_account();
        let (caesar, cctx) = create_account();
        let (brutus, bctx) = create_account();

        let mut erc20 = ERC20Token::new(&gctx, 1000).unwrap();

        // Getafix transfers a sum to Caesar
        let mut transfer = erc20.transfer(&gctx, caesar, 500).unwrap();
        eprintln!("{:?}", transfer);

        let mut balance = erc20.balance_of(&cctx).unwrap();
        assert_eq!(balance, 500u64);

        // Unsuspecting Caesar gives an allowance to Brutus
        let approval = erc20.approve(&cctx, brutus, 400).unwrap();
        eprintln!("{:?}", approval);
        balance = erc20.balance_of(&bctx).unwrap();
        assert_eq!(balance, 0u64);

        // Brutus transfer some tokens from Caesar
        transfer = erc20.transfer_from(&bctx, caesar, brutus, 400).unwrap();
        eprintln!("{:?}", transfer);
        balance = erc20.balance_of(&bctx).unwrap();
        assert_eq!(balance, 400u64);
    }
}
