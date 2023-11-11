//! [ArceOS](https://github.com/rcore-os/arceos) global memory allocator.
//!
//! It provides [`GlobalAllocator`], which implements the trait
//! [`core::alloc::GlobalAlloc`]. A static global variable of type
//! [`GlobalAllocator`] is defined with the `#[global_allocator]` attribute, to
//! be registered as the standard library’s default allocator.

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

#[cfg(not(feature = "early"))]
mod separate_allocator;
#[cfg(feature = "early")]
mod aio_allocator;
mod page;

use allocator::AllocResult;
use core::alloc::Layout;
use core::ptr::NonNull;

const PAGE_SIZE: usize = 0x1000;
#[cfg(not(feature = "early"))]
const MIN_HEAP_SIZE: usize = 0x8000; // 32 K

pub use page::GlobalPage;

cfg_if::cfg_if! {
    if #[cfg(feature = "early")] {
        use crate::aio_allocator::AIOAllocator as DefaultGlobalAllocator;
    } else {
        use crate::separate_allocator::SeparateAllocator as DefaultGlobalAllocator;
    }
}

pub trait GlobalAllocator {
    /// Returns the name of the allocator.
    fn name(&self) -> &'static str;
    /// Initializes the allocator with the given region.
    fn init(&self, start_vaddr: usize, size: usize);
    /// Add the given region to the allocator.
    fn add_memory(&self, start_vaddr: usize, size: usize) -> AllocResult;

    /// Allocate arbitrary number of bytes. Returns the left bound of the
    /// allocated region.
    fn alloc(&self, layout: Layout) -> AllocResult<NonNull<u8>>;
    /// Gives back the allocated region to the byte allocator.
    fn dealloc(&self, pos: NonNull<u8>, layout: Layout);
    /// Allocates contiguous pages.
    fn alloc_pages(&self, num_pages: usize, align_pow2: usize) -> AllocResult<usize>;
    /// Gives back the allocated pages starts from `pos` to the page allocator.
    fn dealloc_pages(&self, pos: usize, num_pages: usize);

    /// Returns the number of allocated bytes in the byte allocator.
    fn used_bytes(&self) -> usize;
    /// Returns the number of available bytes in the byte allocator.
    fn available_bytes(&self) -> usize;
    /// Returns the number of allocated pages in the page allocator.
    fn used_pages(&self) -> usize;
    /// Returns the number of available pages in the page allocator.
    fn available_pages(&self) -> usize;
}

#[cfg_attr(all(target_os = "none", not(test)), global_allocator)]
static GLOBAL_ALLOCATOR: DefaultGlobalAllocator = DefaultGlobalAllocator::new();

/// Returns the reference to the global allocator.
pub fn global_allocator() -> &'static DefaultGlobalAllocator {
    &GLOBAL_ALLOCATOR
}

/// Initializes the global allocator with the given memory region.
///
/// Note that the memory region bounds are just numbers, and the allocator
/// does not actually access the region. Users should ensure that the region
/// is valid and not being used by others, so that the allocated memory is also
/// valid.
///
/// This function should be called only once, and before any allocation.
pub fn global_init(start_vaddr: usize, size: usize) {
    debug!(
        "initialize global allocator at: [{:#x}, {:#x})",
        start_vaddr,
        start_vaddr + size
    );
    GLOBAL_ALLOCATOR.init(start_vaddr, size);
}

/// Add the given memory region to the global allocator.
///
/// Users should ensure that the region is valid and not being used by others,
/// so that the allocated memory is also valid.
///
/// It's similar to [`global_init`], but can be called multiple times.
pub fn global_add_memory(start_vaddr: usize, size: usize) -> AllocResult {
    debug!(
        "add a memory region to global allocator: [{:#x}, {:#x})",
        start_vaddr,
        start_vaddr + size
    );
    GLOBAL_ALLOCATOR.add_memory(start_vaddr, size)
}
