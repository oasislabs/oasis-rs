#[macro_use]
extern crate serde; // Provides `Serialize` and `Deserialize`.
extern crate chrono;

pub mod types;

use mantle::{Context, Event, Service};

use chrono::Utc;
use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::types::*;

#[derive(Service, Default)]
pub struct AuctionMarket {
    /// The item counter
    item_counter: u64,

    /// The administrators of this auction mechanism
    /// Administrators can blacklist users without due cause
    admins: HashSet<UserId>,

    /// The blacklist of users
    blacklist: HashSet<UserId>,

    /// The current valuation of the market as the cummulative value
    /// of all artifacts that have been up for auction
    market_size: u64,

    /// The repository of unique artifacts
    artifacts: HashMap<ItemId, Artifact>,

    /// Active auctions
    auctions: HashMap<ItemId, Auction>,
}

impl AuctionMarket {
    /// Constructs a new `AuctionMarket`.
    pub fn new(ctx: &Context) -> Result<Self> {
        let mut admins = HashSet::new();
        admins.insert(ctx.sender());

        Ok(Self {
            admins,
            ..Default::default()
        })
    }

    /// Adds a new auction item to the marketplace
    /// Anyone can sell an item. Once they add an item to the market
    /// they control the auction through its conclusion
    pub fn add_item(&mut self, ctx: &Context, value: u64, description: String) -> Result<Artifact> {
        if self.blacklist.contains(&ctx.sender()) {
            return Err(Error::BlacklistedUser {
                user_id: ctx.sender(),
            });
        }

        // build a new artifact object
        let artifact = Artifact {
            base: ArtifactBase {
                item_id: self.item_counter,
                description: description,
            },
            owner: ctx.sender(),
            provenance: Vec::new(),
            value,
            transaction_time: Utc::now().to_rfc2822(),
        };
        self.artifacts.insert(self.item_counter, artifact.clone());
        self.market_size = self.market_size + value;

        // emit an event to notify watchers that a new item has been added and will
        // be available for auction soon
        Event::emit(&self.artifacts.get(&self.item_counter).unwrap().base);
        self.item_counter = self.item_counter + 1;

        Ok(artifact)
    }
}

impl AuctionMarket {
    /// Get an artifact by item_id
    pub fn get_artifact(&mut self, _ctx: &Context, item_id: ItemId) -> Result<Artifact> {
        if !self.artifacts.contains_key(&item_id) {
            return Err(Error::InvalidItem {
                item_id,
            });
        }
        // If the item is currently in auction then you cannot get it
        if self.auctions.contains_key(&item_id) {
            return Err(Error::ItemInLiveAuction {
                item_id,
            });
        }
        let artifact = self.artifacts.get(&item_id).unwrap();
        Ok(artifact.clone())
    }

    /// Get current market size
    pub fn get_market_size(&mut self, _ctx: &Context) -> Result<u64> {
        Ok(self.market_size)
    }
}

impl AuctionMarket {
    /// Start an auction for an item
    pub fn start_auction(
        &mut self,
        ctx: &Context,
        item_id: ItemId,
        reserve: u64,
    ) -> Result<Auction> {
        // check that the item is owned by the person who wants to start the auction
        let artifact = match self.artifacts.entry(item_id) {
            Entry::Vacant(ve) => return Err(Error::InvalidItem {
                item_id: *ve.key(),
            }),
            Entry::Occupied(oe) => oe.into_mut(),
        };
        if artifact.owner != ctx.sender() {
            return Err(Error::InvalidOwner {
                item_id,
            });
        }
        // if the item is already being auctioned then err
        if self.auctions.contains_key(&item_id) {
            return Err(Error::ItemInLiveAuction {
                item_id,
            });
        }

        // build a new auction object
        let auction = Auction {
            item_id,
            seller: ctx.sender(),
            reserve: reserve,
            ..Default::default()
        };
        self.auctions.insert(item_id, auction.clone());

        // emit an event for the start of this auction
        artifact.base.emit();

        Ok(auction)
    }

    /// Place a bid on an item
    /// Anyone other than the seller can place a bid as long as they are not
    /// on the blacklist
    pub fn place_bid(&mut self, ctx: &Context, item_id: ItemId, value: u64) -> Result<()> {
        let auction = match self.auctions.entry(item_id) {
            Entry::Vacant(ve) => {
                return Err(Error::ItemNotActive {
                    item_id: *ve.key(),
                })
            }
            Entry::Occupied(oe) => oe.into_mut(),
        };
        if auction.seller == ctx.sender() {
            return Err(Error::SellerBidderIndistinct);
        }
        if self.blacklist.contains(&ctx.sender()) {
            return Err(Error::BlacklistedUser {
                user_id: ctx.sender(),
            });
        }
        if value < auction.reserve {
            return Err(Error::ReserveNotMet {
                reserve: auction.reserve,
            });
        }
        if value < auction.max_bid {
            return Err(Error::Outbid);
        }

        // Create/update bid
        if !auction.bids.contains_key(&ctx.sender()) {
            auction.bids.insert(
                ctx.sender(),
                Bid {
                    bidder: ctx.sender(),
                    value: value,
                },
            );
        } else {
            let mut bid = auction.bids.get_mut(&ctx.sender()).unwrap();
            if bid.value > value || value < auction.max_bid {
                return Err(Error::NonMonotonicBid {
                    value: std::cmp::max(bid.value, auction.max_bid),
                });
            }
            bid.value = value;
        }
        auction.max_bid = value;

        Ok(())
    }

    /// Close an auction
    /// Only the seller can close an auction
    pub fn close_auction(&mut self, ctx: &Context, item_id: ItemId) -> Result<Auction> {
        let mut auction = match self.auctions.entry(item_id) {
            Entry::Vacant(ve) => {
                return Err(Error::ItemNotActive {
                    item_id: *ve.key(),
                })
            }
            Entry::Occupied(_) => self.auctions.remove(&item_id).unwrap(),
        };
        if auction.seller != ctx.sender() {
            return Err(Error::InvalidCloseRequest);
        }
        if auction.bids.keys().len() == 0 {
            return Err(Error::InsufficientBids {
                item_id,
            });
        }

        // get artifact
        let mut artifact = self.artifacts.get_mut(&item_id).unwrap();

        // sort the bids and pick the winner and price
        let mut bids_vec: Vec<_> = auction.bids.values().collect();
        bids_vec.sort_by(|l, r| r.value.cmp(&l.value));

        let winner = &bids_vec[0];
        auction.buyer = winner.bidder;
        if bids_vec.len() > 1 {
            auction.realized = bids_vec[1].value;
        } else {
            auction.realized = auction.reserve;
        }
        self.market_size = self.market_size - artifact.value + auction.realized;
        artifact.provenance.push(auction.clone());
        artifact.value = auction.realized;
        let transaction_time = Utc::now().to_rfc2822();
        artifact.transaction_time = transaction_time.clone();

        Event::emit(&Summary {
            owner: auction.buyer,
            realized: auction.realized,
            item_id: auction.item_id,
            transaction_time,
        });

        Ok(auction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use mantle::{Address, Context};

    /// Creates a new account and a `Context` with the new account as the sender.
    fn create_account() -> (Address, Context) {
        let addr = mantle_test::create_account(0 /* initial balance */);
        let ctx = Context::default().with_sender(addr).with_gas(100_000);
        (addr, ctx)
    }

    #[test]
    fn happy_path() {
        let (_getafix, gctx) = create_account();
        let (_fulliautomatix, fctx) = create_account();
        let (_caesar, cctx) = create_account();
        let (_brutus, bctx) = create_account();

        let mut am = AuctionMarket::new(&gctx).unwrap();

        // Fulliautomatix the village blacksmith adds an item
        let artifact = am
            .add_item(&fctx, 100, "Roman Leewen Pugio Dagger".to_string())
            .unwrap();
        std::eprintln!("{:?}", artifact);
        assert_eq!(artifact.base.item_id, 0);
        assert_eq!(artifact.base.description, "Roman Leewen Pugio Dagger");
        assert_eq!(artifact.value, 100);

        // He then starts an auction for his item
        let auction = am.start_auction(&fctx, artifact.base.item_id, 100).unwrap();
        std::eprintln!("Auction start state: {:?}", auction);

        // Caesar bids
        match am.place_bid(&cctx, artifact.base.item_id, 200) {
            Ok(_) => assert!(true),
            Err(e) => {
                std::println!("ERROR: {}", e);
                assert!(false);
            }
        };
        // Brutus bids to which Caesar remarks "Et tu, Brute?"
        match am.place_bid(&bctx, artifact.base.item_id, 400) {
            Ok(_) => assert!(true),
            Err(e) => {
                std::println!("ERROR: {}", e);
                assert!(false);
            }
        }

        // Fulliautomatix, tired of living under the Roman yoke, closes the auction
        let closed_auction = am.close_auction(&fctx, artifact.base.item_id).unwrap();
        std::eprintln!("Auction close state: {:?}", closed_auction);

        assert_eq!(closed_auction.buyer, _brutus);
        assert_eq!(closed_auction.realized, 200);

        // And the rest is history
    }

    #[test]
    fn many_unhappy_paths() {
        let (_getafix, gctx) = create_account();
        let (_fulliautomatix, fctx) = create_account();
        let (_caesar, cctx) = create_account();
        let (_brutus, bctx) = create_account();

        let mut am = AuctionMarket::new(&gctx).unwrap();

        // Fulliautomatix the village blacksmith adds an item
        let artifact = am
            .add_item(&fctx, 100, "Roman Leewen Pugio Dagger".to_string())
            .unwrap();
        std::eprintln!("{:?}", artifact);

        // Someone other than fulliautomatix attempts to start the auction
        match am.start_auction(&bctx, artifact.base.item_id, 100) {
            Ok(_) => assert!(false),
            Err(e) => {
                std::eprintln!("ERROR: {}", e);
                assert!(true);
            }
        };

        // fulliautomatix starts the auction
        let auction = am.start_auction(&fctx, artifact.base.item_id, 100).unwrap();
        std::eprintln!("Auction start state: {:?}", auction);

        // Parsimonious Caesar bids
        match am.place_bid(&cctx, artifact.base.item_id, 10) {
            Ok(_) => assert!(false),
            Err(e) => {
                std::eprintln!("ERROR: {}", e);
                assert!(true);
            }
        }

        // Caesar revises his bid
        match am.place_bid(&cctx, artifact.base.item_id, 300) {
            Ok(_) => assert!(true),
            Err(e) => {
                std::println!("ERROR: {}", e);
                assert!(false);
            }
        };
        // Brutus thinks he knows the mind of Caesar, the son of a she-wolf
        match am.place_bid(&bctx, artifact.base.item_id, 200) {
            Ok(_) => assert!(false),
            Err(e) => {
                std::println!("ERROR: {}", e);
                assert!(true);
            }
        }

        // Brutus quickly re-bids lest he loses his precious cargo
        match am.place_bid(&bctx, artifact.base.item_id, 400) {
            Ok(_) => assert!(true),
            Err(e) => {
                std::println!("ERROR: {}", e);
                assert!(false);
            }
        }

        let two_seconds = Duration::new(2, 0);
        std::thread::sleep(two_seconds);

        // Fulliautomatix, tired of living under the Roman yoke, siezes the opportunity and
        // closes the auction
        let closed_auction = am.close_auction(&fctx, artifact.base.item_id).unwrap();
        std::eprintln!("Auction close state: {:?}", closed_auction);

        assert_eq!(closed_auction.buyer, _brutus);
        assert_eq!(closed_auction.realized, 300);

        std::eprintln!(
            "{:?}",
            am.get_artifact(&fctx, artifact.base.item_id).unwrap()
        );
        std::eprintln!(
            "Market size = {:?}",
            am.get_market_size(&fctx).unwrap()
        );
    }
}
