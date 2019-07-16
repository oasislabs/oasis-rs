#[macro_use]
extern crate serde;

use map_vec::{map::Entry, Map};
use oasis_std::{Context, Service};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedLanguage,
    DuplicateEntry,
}

#[derive(Service)]
pub struct HelloWorld {
    helloworlds: Map<String, String>,
}

impl HelloWorld {
    /// Constructs a new `HelloWorld` service
    pub fn new(_ctx: &Context) -> Result<Self> {
        let helloworlds = vec![
            ("en", "Hello, world!"),
            ("sl", "Pozdravljen, svet!"),
            ("de", "Hello Welt!"),
            ("fr", "Bonjour le monde!"),
        ]
        .into_iter()
        .map(|(lang, hello)| (lang.to_string(), hello.to_string()))
        .collect();

        Ok(Self { helloworlds })
    }

    /// Get hello world taking as input the desired language
    pub fn say_hello(&mut self, _ctx: &Context, language: String) -> Option<&String> {
        self.helloworlds.get(&language)
    }

    /// Add a new language Hello World! pair
    pub fn add_hello(
        &mut self,
        _ctx: &Context,
        language: String,
        helloworld: String,
    ) -> Result<()> {
        match self.helloworlds.entry(language) {
            Entry::Vacant(vacant) => vacant.insert(helloworld),
            Entry::Occupied(_) => return Err(Error::DuplicateEntry),
        };
        Ok(())
    }
}

fn main() {
    oasis_std::service!(HelloWorld);
}

#[cfg(test)]
mod tests {
    use super::*;
    use oasis_std::{Address, Context};

    /// Creates a new account and a `Context` with the new account as the sender.
    fn create_account() -> (Address, Context) {
        let addr = oasis_test::create_account(0 /* initial balance */);
        let ctx = Context::default().with_sender(addr).with_gas(100_000);
        (addr, ctx)
    }

    #[test]
    fn test_paths() {
        let (_me, ctx) = create_account();

        let mut helloworld = HelloWorld::new(&ctx).unwrap();

        // one happy path
        eprintln!(
            "In Slovenian: {:?}",
            helloworld.say_hello(&ctx, "sl".to_string()).unwrap()
        );

        // double unhappiness
        eprintln!(
            "In Samoan: {:?}",
            helloworld.say_hello(&ctx, "ws".to_string())
        );
        eprintln!(
            "{:?}",
            helloworld.add_hello(&ctx, "en".to_string(), "Zeno World!".to_string())
        );

        // let's fix it
        match helloworld.add_hello(
            &ctx,
            "ws".to_string(),
            "alofa fiafia i le lalolagi!".to_string(),
        ) {
            Err(_) => eprintln!("Attempt to insert a duplicate entry."),
            Ok(_) => (),
        };

        // and test it
        let in_samoan = helloworld.say_hello(&ctx, "ws".to_string()).unwrap();
        eprintln!("In Samoan: {:?}", in_samoan);
        assert_eq!(in_samoan, "alofa fiafia i le lalolagi!");
    }
}
