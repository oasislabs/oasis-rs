use std::cell::UnsafeCell;

use crate::{
    ext::{get_bytes, sender},
    types::{Address, H256},
};

/// A type that can be stored in Oasis Storage.
pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Contract {
    /// Builds a contract struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a contract struct to Storage.
    fn sunder(c: Self);
}

/// The context of the current RPC call.
pub struct Context {}

impl Context {
    pub fn sender(&self) -> Address {
        sender()
    }
}

/// Container for contrat state that is lazily loaded from storage.
/// Currently can only be used as a top-level type (e.g., `Lazy<Vec<T>>`, not `Vec<Lazy<T>>`).
///
/// ## Example
///
/// ```
/// oasis_std::contract! {
/// #[derive(Contract)]
/// pub struct SinglePlayerRPG {
///     player_name: String,
///     inventory: Vec<InventoryItem>,
///     bank: Lazy<HashMap<InventoryItem, u64>>,
/// }
///
/// impl SinglePlayerRPG {
///    pub fn new(player_name: String) -> Self {
///        Self {
///           player_name,
///           inventory: Vec::new(),
///           bank: lazy!(HashMap::new()),
///        }
///    }
///
///    pub fn get_inventory(&self) -> Vec<InventoryItem> {
///        self.inventory.clone()
///    }
///
///    pub fn get_bank(&self) -> Vec<InventoryItem> {
///        self.bank.get().clone()
///    }
///
///    pub fn move_item_to_inventory(&mut self, item: InventoryItem) {
///        self.bank.get_mut().entry(&item).and_modify(|count| {
///            if count > 0 {
///                 self.inventory.push(item);
///                 *count -= 1
///            }
///        });
///    }
/// }
/// }
/// ```
#[derive(Debug)]
pub struct Lazy<T: Storage> {
    key: H256,
    val: UnsafeCell<Option<T>>,
}

impl<T: Storage> Lazy<T> {
    /// Creates a Lazy value with initial contents.
    /// This function is for internal use. Clients should use the `lazy!` macro.
    pub fn new(key: H256, val: T) -> Self {
        Self {
            key,
            val: UnsafeCell::new(Some(val)),
        }
    }

    pub fn get(&self) -> &T {
        let val = unsafe { &mut *self.val.get() };
        if val.is_none() {
            val.replace(serde_cbor::from_slice(&get_bytes(&self.key).unwrap()).unwrap());
        }
        val.as_ref().unwrap()
    }
}

// in reality, this is just a marker for inserting a `Lazy::new`
#[macro_export]
macro_rules! lazy {
    ($val:expr) => {
        compile_error!("`lazy!` used outside of `return Self { ... }`")
    };
}
