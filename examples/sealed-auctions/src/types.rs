extern crate chrono;

use std::{collections::HashMap, vec::Vec};

use mantle::{Address, Event};

pub type ItemId = u64;
pub type Result<T> = std::result::Result<T, Error>;
pub type UserId = Address;

#[derive(Debug, Eq, PartialEq, failure::Fail, Serialize, Deserialize)]
pub enum Error {
    #[fail(display = "Unknown error occured.")]
    Unknown,

    #[fail(display = "The marketplace is not open to {:?}.", user_id)]
    BlacklistedUser { user_id: UserId },

    #[fail(display = "Item {} does not exist.", item_id)]
    InvalidItem { item_id: ItemId },

    #[fail(display = "The item {} is already in a live auction.", item_id)]
    ItemInLiveAuction { item_id: ItemId },

    #[fail(display = "You don't own item {} and hence cannot sell it.", item_id)]
    InvalidOwner { item_id: ItemId },

    #[fail(display = "There is no active auction for item {}.", item_id)]
    ItemNotActive { item_id: ItemId },

    #[fail(display = "The bid has to be higher than the reserve of {}.", reserve)]
    ReserveNotMet { reserve: u64 },

    #[fail(display = "The bid is lower than the current maximum.")]
    Outbid,

    #[fail(display = "A new bid has to be greater than {}.", value)]
    NonMonotonicBid { value: u64 },

    #[fail(display = "The seller is the only one who can close an auction.")]
    InvalidCloseRequest,

    #[fail(display = "There are no bids on item {}. Closing auction.", item_id)]
    InsufficientBids { item_id: ItemId },

    #[fail(display = "The seller cannot also be a bidder for an item.")]
    SellerBidderIndistinct,
}

// A bid struct with the bidder and latest bid
#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
pub struct Bid {
    pub bidder: UserId,
    pub value: u64,
}

impl PartialEq for Bid {
    fn eq(&self, other: &Bid) -> bool {
        self.bidder == other.bidder
    }
}

impl PartialOrd for Bid {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl std::hash::Hash for Bid {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bidder.hash(state);
    }
}

impl std::borrow::Borrow<UserId> for Bid {
    fn borrow(&self) -> &UserId {
        &self.bidder
    }
}

// The auction struct for every auction that has taken place on the platform
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Auction {
    pub item_id: ItemId,
    pub bids: HashMap<UserId, Bid>,
    pub reserve: u64,
    pub max_bid: u64,
    pub realized: u64,
    pub seller: UserId,
    pub buyer: UserId,
}

// We want to emit an event when a new item comes into the auction market
// but don't want to expose anything in the event other than the itemID
// and description. Hence, this struct carries `Event`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Event)]
pub struct ArtifactBase {
    #[indexed]
    pub item_id: ItemId,
    #[indexed]
    pub description: String,
}

// The complete artifact struct that extends the one above with private
// fields that we don't want emited
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Artifact {
    pub base: ArtifactBase,
    pub owner: UserId,
    pub provenance: Vec<Auction>,
    pub value: u64,
    pub transaction_time: String,
}

// An end of auction summary struct
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Event)]
pub struct Summary {
    #[indexed]
    pub owner: UserId,
    #[indexed]
    pub realized: u64,
    #[indexed]
    pub item_id: ItemId,
    pub transaction_time: String,
}
