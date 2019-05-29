#![feature(proc_macro_hygiene)]

use std::collections::{HashMap, HashSet}; // `use` statements can go anywhere

#[oasis_std::service]
mod service {
    pub type UserId = Address;
    pub type PostId = usize;

    #[derive(Service, Default)]
    pub struct MessageBoard {
        /// The administrators of this message board.
        /// Administrators can add and remove users.
        admins: HashSet<UserId>,

        /// The maximum number of characters in a post. For instance, 280.
        bcast_char_limit: Option<u32>,

        /// All of the posts made to this message board.
        posts: Vec<Post>,

        /// All accounts which have ever participated in this message board.
        accounts: HashMap<UserId, Account>, // Strictly speaking, a `Vec` would probably be faster
                                            // since it'd be short lived and algorithmically has
                                            // smaller constant factors. The API of `HashMap` is
                                            // better suited for this field, though.
    }

    // Types used in the state struct must derive serde `Serialize` and `Deserialize`
    // so that they can be persisted and loaded from storage. They must also derive `Clone`
    // (and optionally `Copy`) so that service RPC methods can accept borrowed data which
    // improves deserialization performance.
    //
    // Types do not need to be defined in the same module as the service.
    #[derive(Serialize, Deserialize, Clone)]
    pub struct Post {
        author: UserId,
        text: String,
        comments: Vec<Message>,
    }

    #[derive(Serialize, Deserialize, Clone, Default)]
    pub struct Account {
        inbox: Vec<Message>,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Message {
        from: UserId,
        text: String,
    }

    // An event is a struct that derives `Event`. Calling `Event::emit` on the struct
    // will generate a log that can be picked up by anyone watching the blockchain.
    // Annotating a field with `#[indexed]` will allow clients to efficiently search for
    // events with a particular value of the indexed field. Up to three events may be indexed.
    //
    // A confidential application will want to encrypt the contents of the event to a
    // particular recipent.
    // A highly confidential application will probably not want to emit events at all since
    // the fact that an event was even emitted can leak information. (It can be done using
    // techniques from oblivious transfer, but it requires extreme care to do properly.)
    #[derive(Serialize, Deserialize, Event)]
    pub struct MessagePosted {
        #[indexed]
        pub author: UserId,
        #[indexed]
        pub recipient: Option<UserId>,
    }

    impl MessageBoard {
        /// Constructs a new `MessageBoard`.
        pub fn new(ctx: &Context, bcast_char_limit: Option<u32>) -> Result<Self> {
            let mut admins = HashSet::new();
            admins.insert(ctx.sender());

            Ok(Self {
                admins,
                bcast_char_limit,
                ..Default::default()
            })
        }

        /// Adds a user to this message board.
        /// Can only be used by an admin.
        pub fn add_user(&mut self, ctx: &Context, user_id: UserId) -> Result<()> {
            if !self.admins.contains(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }
            self.accounts.entry(user_id).or_default();
            Ok(())
        }

        /// Removes a user from this message board.
        /// Can only be used by an admin.
        pub fn remove_user(&mut self, ctx: &Context, user_id: UserId) -> Result<()> {
            if !self.admins.contains(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }
            self.accounts.remove(&user_id);
            Ok(())
        }
    }

    impl MessageBoard {
        /// Make a post to this message board. Only registered accounts may post.
        /// Returns the id of the post.
        pub fn post(&mut self, ctx: &Context, text: String) -> Result<usize> {
            if !self.accounts.contains_key(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }

            if let Some(bcast_char_limit) = self.bcast_char_limit {
                if text.len() > bcast_char_limit as usize {
                    return Err(failure::format_err!(
                        "Message of {} characters exeeds limit of {}.",
                        text.len(),
                        bcast_char_limit
                    ));
                }
            }

            self.posts.push(Post {
                author: ctx.sender(),
                text,
                comments: Vec::new(),
            });

            Event::emit(&MessagePosted {
                author: ctx.sender(),
                recipient: None,
            });

            Ok(self.posts.len() - 1)
        }

        /// Returns all posts (optionally: made since a given post).
        pub fn get_posts(&self, ctx: &Context, since: Option<PostId>) -> Result<Vec<Post>> {
            if !self.accounts.contains_key(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }

            let since = since.unwrap_or_default();
            if since >= self.posts.len() {
                return Err(failure::format_err!("Invalid post ID."));
            }

            Ok(self.posts[since..].to_vec())
        }

        /// Add a comment to a post.
        pub fn comment(&mut self, ctx: &Context, post_id: PostId, text: String) -> Result<()> {
            if !self.accounts.contains_key(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }
            if post_id >= self.posts.len() {
                return Err(failure::format_err!("Invalid post ID."));
            }
            self.posts[post_id].comments.push(Message {
                from: ctx.sender(),
                text,
            });
            Ok(())
        }
    }

    impl MessageBoard {
        pub fn send_dm(&mut self, ctx: &Context, recipient: UserId, text: String) -> Result<()> {
            if !self.accounts.contains_key(&ctx.sender()) {
                return Err(failure::format_err!("Permission denied."));
            }
            match self.accounts.get_mut(&recipient) {
                Some(recip) => {
                    recip.inbox.push(Message {
                        from: ctx.sender(),
                        text,
                    });
                    Event::emit(&MessagePosted {
                        author: ctx.sender(),
                        recipient: None,
                    });
                    Ok(())
                }
                None => Err(failure::format_err!("No such user: {}", recipient)),
            }
        }

        /// Retrieves all messages from the sender's inbox. The inbox is emptied by this operation.
        pub fn fetch_inbox(&mut self, ctx: &Context) -> Result<Vec<Message>> {
            Ok(self
                .accounts
                .get_mut(&ctx.sender())
                .map(|acct| std::mem::replace(&mut acct.inbox, Vec::new()))
                .unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a new account and a `Context` with the new account as the sender.
    fn create_account() -> (Address, Context) {
        let addr = oasis_test::create_account(0 /* initial balance */);
        let ctx = Context::default().with_sender(addr).with_gas(100_000);
        (addr, ctx)
    }

    #[test]
    fn functionality() {}
}
