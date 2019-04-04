#![feature(proc_macro_hygiene)]
#[oasis_std::contract]
mod contract {
    #[derive(Contract, Debug)]
    pub struct Forum {
        name: (String),
        users: Vec<User>,
        posts: Vec<ForumPost>,
        chats: Lazy<std::collections::HashMap<(UserId, UserId), Vec<String>>>,
        expiry: (u64, u64), // post, message
    }

    type UserId = Address;

    // in an ideal world, the macro would:
    // 1. collect all structs defined inside of `contract!`,
    // 2. recursively find all types used in the state
    // 3. add the #[derive(Serialize, Deserialize)] if it doesn't exist
    #[derive(Clone, Serialize, Deserialize, Debug, Default)]
    pub struct User {
        id: UserId,
        name: String,
        reputation: i64,
        bounty: U256,
    }

    #[derive(Clone, Serialize, Deserialize, Debug, Default)]
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
        pub fn new(
            ctx: &Context,
            forum_name: String,
            expiry: (u64, u64),
            admin_username: String,
        ) -> Result<Self> {
            // Default::default() is not yet possible because Lazy can't `impl Default`
            // this can be solved using a const generic when those are implemented: `Lazy<T, "key">`
            Ok(Self {
                name: forum_name,
                users: vec![User {
                    id: ctx.sender(),
                    name: admin_username,
                    reputation: 9001,
                    ..Default::default()
                }],
                posts: Vec::new(),
                chats: lazy!(std::collections::HashMap::new()),
                expiry,
            })
        }

        pub fn signup(&mut self, ctx: &Context, name: &str) -> Result<()> {
            self.users.push(User {
                id: ctx.sender(),
                name: name.to_string(),
                ..Default::default()
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

        pub fn give_bounty(&mut self, ctx: &Context, recipient: &UserId) -> Result<()> {
            if ctx.value() == U256::zero() {
                return Ok(());
            }
            match self.users.iter_mut().find(|user| user.id == *recipient) {
                Some(user) => {
                    user.bounty += ctx.value();
                    Ok(())
                }
                None => Err(failure::format_err!("No such user.")),
            }
        }

        pub fn collect_bounty(&mut self, ctx: &Context) -> Result<U256> {
            match self.users.iter_mut().find(|user| user.id == ctx.sender()) {
                Some(user) => {
                    let bounty = user.bounty;
                    user.bounty = U256::zero();
                    user.id.transfer(&bounty)?;
                    Ok(bounty)
                }
                None => Err(failure::format_err!("No such user.")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_user(bb: &Forum, user_id: &UserId, checker: &Fn(&User) -> ()) {
        checker(bb.users.iter().find(|user| user.id == *user_id).unwrap());
    }

    #[test]
    fn test_forum() {
        let admin = oasis_test::create_account(42);
        let boarhunter = oasis_test::create_account(1);

        let admin_ctx = Context::default().with_sender(admin);
        let boarhunter_ctx = Context::default().with_sender(boarhunter);

        let mut bb = Forum::new(
            &admin_ctx,
            "c10l forum".to_string(),
            (90, 01),
            "admin".to_string(),
        )
        .unwrap();

        // sign up boarhunter
        let username = "boarhunter69".to_string();
        bb.signup(&boarhunter_ctx, &username).unwrap();
        check_user(&bb, &boarhunter, &|user| {
            assert_eq!(user.name, username);
            assert_eq!(user.reputation, 0);
        });

        // make post as boarhunter
        bb.post(
            &boarhunter_ctx,
            "Rust is the best".to_string(),      // title
            "👆 title says it al".to_string(), // message
        )
        .unwrap();
        check_user(&bb, &boarhunter, &|user| assert_eq!(user.reputation, 1));

        // send message admin -> boarhunter, boarhunter checks that it was delivered
        bb.dm(&admin_ctx, &boarhunter, "+1").unwrap();
        match bb.get_chats(&boarhunter_ctx, &Some(admin)).unwrap() {
            Either::Left(ref chats) if chats.len() == 1 => {
                assert_eq!(chats[0], "+1");
            }
            ow => panic!("bad chats: {:?}", ow),
        }

        bb.give_bounty(&admin_ctx.with_value(42), &boarhunter)
            .unwrap();
        check_user(&bb, &boarhunter, &|user| assert_eq!(user.bounty, 42u64));
        assert_eq!(admin.balance(), 0u64);
        assert!(bb
            .give_bounty(&admin_ctx.with_value(1), &boarhunter)
            .is_err());
        assert_eq!(boarhunter.balance(), 1u64);

        bb.collect_bounty(&boarhunter_ctx).unwrap();
        check_user(&bb, &boarhunter, &|user| assert_eq!(user.bounty, 0u64));
        assert_eq!(boarhunter.balance(), 43u64);
    }
}
