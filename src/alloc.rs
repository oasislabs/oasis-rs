use core::alloc::{GlobalAlloc, Layout};

pub(crate) struct PlatformAllocator;

extern "C" {
    fn alloc(size: usize, align: usize) -> *mut u8;
    fn free(ptr: *mut u8);
}

unsafe impl GlobalAlloc for PlatformAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr);
    }

    unsafe fn alloc_zeroed(&self, _layout: Layout) -> *mut u8 {
        unimplemented!(); // don't use default impl, but it's not clear if this is even used
    }

    unsafe fn realloc(&self, _ptr: *mut u8, _layout: Layout, _new_size: usize) -> *mut u8 {
        unimplemented!(); // don't use default impl, but it's not clear if this is even used
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: PlatformAllocator = PlatformAllocator;
