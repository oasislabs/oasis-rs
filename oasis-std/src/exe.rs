use std::cell::UnsafeCell;

use crate::types::Address;

/// A type that can be stored in Oasis Storage.
pub trait Storage = serde::Serialize + serde::de::DeserializeOwned;

pub trait Service {
    /// Builds a service struct from items in Storage.
    fn coalesce() -> Self;

    /// Stores a service struct to Storage.
    fn sunder(c: Self);
}

pub trait Event {
    /// A struct implementing the builder pattern for setting topics.
    ///
    /// For example,
    /// ```
    /// #[derive(Event)]
    /// struct MyEvent {
    ///    #[indexed]
    ///    my_topic: u64,
    ///    #[indexed]
    ///    my_other_topic: u64,
    /// }
    ///
    /// let topics: Vec<Vec<u8>> = MyTopics::Topics::default()
    ///    .set_my_other_topic(42)
    ///    .hash();
    /// // topics = vec![0, keccak256(abi_encode(my_other_topic))]
    /// ```
    type Topics;

    /// Emits an event tagged with the (keccak) hashed function name and topics.
    fn emit(&self);
}

/// The context of the current RPC.
// `Option` values are set by the user. `None` when populated by runting (during call/deploy).
#[derive(Default, Copy, Clone, Debug)]
pub struct Context {
    #[doc(hidden)]
    pub sender: Option<Address>,

    #[doc(hidden)]
    pub value: Option<u64>,

    #[doc(hidden)]
    pub gas: Option<u64>,

    #[doc(hidden)]
    pub call_type: CallType,
}

#[derive(Copy, Clone, Debug)]
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

    /// Sets the sender of the RPC receiving this `Context` as an argument.
    /// Has no effect when called inside of a service.
    pub fn with_sender(mut self, sender: Address) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Amends a Context with the value that should be transferred to the callee.
    pub fn with_value(mut self, value: u64) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets the amount of computation resources available to the callee.
    /// Payed for by the `payer` of the `Context`.
    pub fn with_gas(mut self, gas: u64) -> Self {
        self.gas = Some(gas.into());
        self
    }

    /// Returns the `Address` of the sender of the current RPC.
    pub fn sender(&self) -> Address {
        self.sender.unwrap_or_else(ext::sender)
    }

    /// Returns the `Address` of the currently executing service.
    /// Panics if not currently in a service.
    pub fn address(&self) -> Address {
        ext::address()
    }

    /// Returns the value with which this `Context` was created.
    pub fn value(&self) -> u64 {
        self.value.unwrap_or_else(ext::value)
    }

    /// Returns the remaining gas allocated to this transaction.
    pub fn gas_left(&self) -> u64 {
        ext::gas_left()
    }
}

/// Container for service state that is lazily loaded from storage.
/// Currently can only be used as a top-level type (e.g., `Lazy<Vec<T>>`, not `Vec<Lazy<T>>`).
/// where the entire Vec will be lazily instantiated (as opposed to each individual element).
///
/// ## Example
///
/// ```
/// oasis_std::service! {
/// #[derive(Service)]
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
    key: Vec<u8>,
    val: UnsafeCell<Option<T>>,
}

impl<T: Storage> Lazy<T> {
    /// Creates a Lazy value with initial contents.
    /// This function is for internal use. Clients should use the `lazy!` macro.
    #[doc(hidden)]
    pub fn _new(key: Vec<u8>, val: T) -> Self {
        Self {
            key,
            val: UnsafeCell::new(Some(val)),
        }
    }

    /// Creates an empty Lazy. This function is for internal use.
    #[doc(hidden)]
    pub fn _uninitialized(key: Vec<u8>) -> Self {
        Self {
            key,
            val: UnsafeCell::new(None),
        }
    }

    fn ensure_val(&self) -> &mut T {
        let val = unsafe { &mut *self.val.get() };
        if val.is_none() {
            val.replace(serde_cbor::from_slice(&ext::get_bytes(&self.key).unwrap()).unwrap());
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
///        the_field: Lazy::new(keccak256("the_field".as_bytes().to_vec()), the_val)
///    }
/// }
/// ```
#[macro_export]
macro_rules! lazy {
    ($val:expr) => {
        compile_error!("`lazy!` used outside of struct expr.")
    };
}
