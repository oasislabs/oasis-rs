oasis_macros::contract! { // TODO: rustfmt needs to work inside of macros

use std::collections::HashMap;

use either::Either;

#[derive(Contract)]
pub struct Forum {
    users: Vec<User>,
    posts: Vec<ForumPost>,
    chats: Lazy<HashMap<(UserId, UserId), Vec<String>>>,
}

type UserId = Address;

// in an ideal world, the macro would:
// 1. collect all structs defined inside of `contract!`,
// 2. recursively find all types used in the state
// 3. add the #[derive(Serialize, Deserialize)] if it doesn't exist
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    id: UserId,
    name: String,
    reputation: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ForumPost {
    author: UserId,
    title: String,
    message: String,
    replies: Vec<String>,
}

impl Forum {
    pub fn new(_ctx: Context) -> Self {
        // Default::default() is not yet possible because Lazy can't `impl Default`
        // this can be solved using a const generic when those are implemented: `Lazy<T, "key">`
        Self {
            users: Vec::new(),
            posts: Vec::new(),
            chats: lazy!(HashMap::new()),
        }
    }

    pub fn signup(&mut self, ctx: Context, name: String) {
        self.users.push(User {
            id: ctx.sender(),
            name,
            reputation: 0,
        })
    }

    pub fn post(&mut self, ctx: Context, title: String, message: String) {
        if let Some(mut user) = self.users.iter_mut().find(|user| user.id == ctx.sender()) {
            self.posts.push(ForumPost {
                author: ctx.sender(),
                title,
                message,
                replies: Vec::new(),
            });
            user.reputation += 1;
        }
    }

    pub fn get_posts(&self, ctx: Context) -> Vec<&ForumPost> {
        self.users
            .iter()
            .find(|user| user.id == ctx.sender())
            .map(|_| self.posts.iter().collect()) // doesn't actually need `clone` since it'll be serialized
            .unwrap_or(Vec::new())
    }

    pub fn dm(&mut self, ctx: Context, to: UserId, message: String) {
        self.chats
            .get_mut()
            .entry((ctx.sender(), to))
            .or_default()
            .push(message)
    }

    pub fn get_chats(
        &self,
        ctx: Context,
        with: Option<UserId>,
    ) -> Either<Vec<&String>, Vec<(&UserId, &Vec<String>)>> {
        match self.users.iter().find(|user| user.id == ctx.sender()) {
            Some(_) => match with {
                Some(with) => Either::Left(
                    self.chats
                        .get()
                        .get(&(ctx.sender(), with))
                        .map(|chats| chats.iter().collect())
                        .unwrap_or(Vec::new()),
                ),
                None => Either::Right(
                    self.chats
                        .get()
                        .iter()
                        .filter_map(|((from, to), messages)| {
                            if from == &ctx.sender() {
                                Some((to, messages))
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
            },
            None => match with {
                Some(_) => Either::Left(Vec::new()),
                None => Either::Right(Vec::new()),
            },
        }
    }
}

} // TODO: TaG

#[test]
// #[oasis_test::test] // TODO
fn test() {
    use oasis_std::prelude::*;

    let mut ctx = oasis_std::exe::Context::default();
    ctx.set_sender(Address::from([42u8; 20]));

    // TODO: derive client that looks like struct
    let mut bb = Forum::new(ctx.clone());

    let username = "boarhunter69";
    bb.signup(ctx.clone(), username.to_string()); // technically the client can accept AsRef<T>
                                                  // are ergonomics worth the type mismatch?

    macro_rules! find_user {
        () => {
            bb.users
                .iter()
                .find(|user| user.id == ctx.sender())
                .expect("`signup` failed")
        };
    }

    let user = find_user!();
    assert_eq!(user.name, username.to_string());
    assert_eq!(user.reputation, 0);
    std::mem::drop(user);

    let title = "Rust is the best!";
    let message = "👆 title says it all";
    bb.post(ctx.clone(), title.to_string(), message.to_string());

    let user = find_user!();
    assert_eq!(user.reputation, 1);
}
