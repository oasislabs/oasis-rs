use crate::{
    ext::{get_bytes, sender},
    types::{Address, H256},
};

pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Contract<T> {
    fn coalesce() -> T;
    fn sunder(c: T);
}

pub struct Context {}

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
///           bank: Lazy::init(HashMap::new()),
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
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Lazy<T: Storage> {
    val: Option<T>,
}

impl<T: Storage> Lazy<T> {
    pub fn init(val: T) -> Self {
        Self { val: Some(val) }
    }
    pub fn get(&self) -> &T {
        if self.val.is_none() {
            unsafe {
                (*(self as *const Self as *mut Self)).val.replace(
                    serde_cbor::from_slice(&get_bytes(&H256::zero() /* TODO */).unwrap()).unwrap(),
                );
            }
        }
        self.val.as_ref().unwrap()
    }
}

impl Context {
    pub fn sender(&self) -> Address {
        sender()
    }
}
