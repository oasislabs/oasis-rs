use std::cell::UnsafeCell;

use crate::{
    ext::{self, get_bytes, sender},
    types::{Address, H256, U256},
};

/// A type that can be stored in Oasis Storage.
pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Contract {
    /// Builds a contract struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a contract struct to Storage.
    fn sunder(c: Self);
}

/// The context of the current RPC.
#[derive(Default, Copy, Clone)]
pub struct Context {
    #[doc(hidden)]
    pub value: Option<U256>, //User-provider value. `None` when created during call/deploy

    #[doc(hidden)]
    pub call_type: CallType,
}

#[derive(Copy, Clone)]
pub enum CallType {
    Default,
    Delegated,
    Constant,
}

impl Default for CallType {
    fn default() -> Self {
        CallType::Default
    }
}

impl Context {
    pub fn delegated() -> Self {
        Self {
            call_type: CallType::Delegated,
            ..Default::default()
        }
    }

    /// Creates a new Context, based on the current environment, that specifies
    /// a transfer of `value` to the callee.
    pub fn with_value(mut self, value: U256) -> Self {
        self.value = Some(value);
        self
    }

    /// Returns the `Address` of the sender of the current RPC.
    pub fn sender(&self) -> Address {
        sender()
    }

    /// Returns the value with which this `Context` was created.
    pub fn value(&self) -> U256 {
        self.value.unwrap_or_else(ext::value)
    }
}

/// Container for contract state that is lazily loaded from storage.
/// Currently can only be used as a top-level type (e.g., `Lazy<Vec<T>>`, not `Vec<Lazy<T>>`).
/// where the entire Vec will be lazily instantiated (as opposed to each individual element).
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
///    pub fn new(_ctx: &Context, player_name: String) -> Self {
///        Self {
///           player_name,
///           inventory: Vec::new(),
///           bank: lazy!(HashMap::new()),
///        }
///    }
///
///    pub fn get_inventory(&self, _ctx: &Context) -> Vec<InventoryItem> {
///        self.inventory.clone()
///    }
///
///    pub fn get_bank(&self, _ctx: &Context) -> Vec<InventoryItem> {
///        self.bank.get().clone()
///    }
///
///    pub fn move_item_to_inventory(&mut self, _ctx: &Context, item: InventoryItem) {
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
    #[doc(hidden)]
    pub fn _new(key: H256, val: T) -> Self {
        Self {
            key,
            val: UnsafeCell::new(Some(val)),
        }
    }

    /// Creates an empty Lazy. This function is for internal use.
    #[doc(hidden)]
    pub fn _uninitialized(key: H256) -> Self {
        Self {
            key,
            val: UnsafeCell::new(None),
        }
    }

    fn ensure_val(&self) -> &mut T {
        let val = unsafe { &mut *self.val.get() };
        if val.is_none() {
            val.replace(serde_cbor::from_slice(&get_bytes(&self.key).unwrap()).unwrap());
        }
        val.as_mut().unwrap()
    }

    /// Returns a reference to the value loaded from Storage.
    pub fn get(&self) -> &T {
        self.ensure_val()
    }

    /// Returns a mutable reference to the value loaded from Storage.
    pub fn get_mut(&mut self) -> &mut T {
        self.ensure_val()
    }

    pub fn is_initialized(&self) -> bool {
        unsafe { &*self.val.get() }.is_some()
    }
}

/// A marker for inserting a `Lazy::new`.
/// Works in tandem with `oasis_macros::LazyInserter`.
///
/// ```
/// fn new(ctx: &Context) -> Self {
///    Self { the_field: lazy!(the_val) }
/// }
/// ```
/// expands to
/// ```
/// fn new(ctx: &Context) -> Self {
///    Self {
///        the_field: Lazy::new(H256::from(keccak256("the_field".as_bytes())), the_val)
///    }
/// }
/// ```
#[macro_export]
macro_rules! lazy {
    ($val:expr) => {
        compile_error!("`lazy!` used outside of struct expr.")
    };
}
