//! Global allocator implementation using the kernel's slab allocator

use super::slab::{kfree, kmalloc};
use core::alloc::{GlobalAlloc, Layout};

/// Global allocator that uses the kernel's slab allocator
pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let addr = kmalloc(layout.size());
        if addr == 0 {
            core::ptr::null_mut()
        } else {
            addr as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if !ptr.is_null() {
            kfree(ptr as usize);
        }
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;
