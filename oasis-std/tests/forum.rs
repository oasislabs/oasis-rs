#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Debug)]
    pub struct Forum {
        users: Vec<User>,
        posts: Vec<ForumPost>,
        chats: Lazy<std::collections::HashMap<(UserId, UserId), Vec<String>>>,
    }

    type UserId = Address;

    // in an ideal world, the macro would:
    // 1. collect all structs defined inside of `contract!`,
    // 2. recursively find all types used in the state
    // 3. add the #[derive(Serialize, Deserialize)] if it doesn't exist
    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct User {
        id: UserId,
        name: String,
        reputation: i64,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub struct ForumPost {
        author: UserId,
        title: String,
        message: String,
        replies: Vec<String>,
    }

    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub enum Either<L, R> {
        Left(L),
        Right(R),
    }

    impl Forum {
        pub fn new(ctx: &Context, admin_username: String) -> Result<Self> {
            // Default::default() is not yet possible because Lazy can't `impl Default`
            // this can be solved using a const generic when those are implemented: `Lazy<T, "key">`
            Ok(Self {
                users: vec![User {
                    id: ctx.sender(),
                    name: admin_username,
                    reputation: 9001,
                }],
                posts: Vec::new(),
                chats: lazy!(std::collections::HashMap::new()),
            })
        }

        pub fn signup(&mut self, ctx: &Context, name: &str) -> Result<()> {
            self.users.push(User {
                id: ctx.sender(),
                name: name.to_string(),
                reputation: 0,
            });
            Ok(())
        }

        pub fn post(&mut self, ctx: &Context, title: String, message: String) -> Result<()> {
            match self.users.iter_mut().find(|user| user.id == ctx.sender()) {
                Some(mut user) => {
                    self.posts.push(ForumPost {
                        author: ctx.sender(),
                        title,
                        message,
                        replies: Vec::new(),
                    });
                    user.reputation += 1;
                    Ok(())
                }
                None => Err(failure::format_err!("403")),
            }
        }

        pub fn get_posts(&self, ctx: &Context) -> Result<Vec<&ForumPost>> {
            if !self.users.iter().any(|user| user.id == ctx.sender()) {
                return Err(failure::format_err!("403"));
            }
            Ok(self.posts.iter().collect())
        }

        pub fn dm(&mut self, ctx: &Context, to: &UserId, message: &str) -> Result<()> {
            self.chats
                .get_mut()
                .entry((ctx.sender(), *to))
                .or_default()
                .push(message.to_string());
            Ok(())
        }

        pub fn get_chats(
            &self,
            ctx: &Context,
            with: &Option<UserId>,
        ) -> Result<Either<Vec<&String>, Vec<(&UserId, &Vec<String>)>>> {
            match self.users.iter().find(|user| user.id == ctx.sender()) {
                Some(_) => match with {
                    Some(with) => {
                        let sender = ctx.sender();
                        let chats_tuple = if sender > *with {
                            (*with, sender)
                        } else {
                            (sender, *with)
                        };
                        Ok(Either::Left(
                            self.chats
                                .get()
                                .get(&chats_tuple)
                                .map(|chats| chats.iter().collect())
                                .unwrap_or(Vec::new()),
                        ))
                    }
                    None => Ok(Either::Right(
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
                    )),
                },
                None => Err(failure::format_err!("403")),
            }
        }
    }
}

macro_rules! find_user {
    ($bb:ident, $ctx:ident) => {
        $bb.users
            .iter()
            .find(|user| user.id == $ctx.sender())
            .expect("`signup` failed")
    };
}

speculate::speculate! {

    describe "forum" {
        before {
            oasis_test::init!();
        }

        it "should work" {
            use oasis_std::prelude::*;

            let mut ctx = Context::default();

            let admin_addr = Address::from([42u8; 20]);
            ctx.set_sender(admin_addr.clone());
            let mut bb = Forum::new(&ctx, "admin".to_string()).unwrap();

            let username = "boarhunter69";
            let user_addr = Address::from([69u8; 20]);
            ctx.set_sender(user_addr.clone());
            bb.signup(&ctx, username).unwrap();

            let user = find_user!(bb, ctx);
            assert_eq!(user.name, username.to_string());
            assert_eq!(user.reputation, 0);

            let title = "Rust is the best!";
            let message = "👆 title says it all";
            bb.post(&ctx, title.to_string(), message.to_string()).unwrap();

            let user = find_user!(bb, ctx);
            assert_eq!(user.reputation, 1);

            ctx.set_sender(admin_addr.clone());
            bb.dm(&ctx, &user_addr, "+1").unwrap();

            ctx.set_sender(user_addr.clone());
            match bb.get_chats(&ctx, &Some(admin_addr)).unwrap() {
                Either::Left(ref chats) if chats.len() == 1 => {
                    assert_eq!(chats[0],  "+1");
                },
                ow => panic!("bad chats: {:?}", ow)
            }
        }
    }
}
