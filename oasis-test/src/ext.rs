use std::cell::RefCell;

use oasis_std::types::Address;

thread_local!(static SENDER: RefCell<Address> = RefCell::new(Address::zero()));

#[no_mangle]
pub extern "C" fn sender(dest: *mut u8) {
    SENDER.with(|sender| {
        unsafe { dest.copy_from_nonoverlapping(sender.borrow().as_ptr(), 20) };
    });
}

pub(crate) fn set_sender(sender: Address) {
    SENDER.with(|s| {
        *s.borrow_mut() = sender;
    });
}
