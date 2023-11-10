use allocator::{AllocResult, EarlyAllocator as DefaultBPAllocator, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use spinlock::SpinNoIrq;

use super::{PAGE_SIZE, GlobalAllocator};

pub struct AIOAllocator {
    pballoc: SpinNoIrq<DefaultBPAllocator<PAGE_SIZE>>,
}

impl AIOAllocator {
    /// Creates an empty [`AIOAllocator`]
    pub const fn new() -> Self {
        Self {
            pballoc: SpinNoIrq::new(DefaultBPAllocator::new()),
        }
    }
}

impl GlobalAllocator for AIOAllocator {
    /// Returns the name of the allocator.
    fn name(&self) -> &'static str {
        "early"
    }

    /// Initializes the allocator with the given region.
    fn init(&self, start_vaddr: usize, size: usize) {
        self.pballoc.lock().init(start_vaddr, size)
    }

    /// Add the given region to the allocator.
    fn add_memory(&self, start_vaddr: usize, size: usize) -> AllocResult {
        self.pballoc.lock().add_memory(start_vaddr, size)
    }

    /// Allocate arbitrary number of bytes. Returns the left bound of the
    /// allocated region.
    fn alloc(&self, layout: Layout) -> AllocResult<NonNull<u8>> {
        self.pballoc.lock().alloc(layout)
    }

    /// Gives back the allocated region to the byte allocator.
    fn dealloc(&self, pos: NonNull<u8>, layout: Layout) {
        self.pballoc.lock().dealloc(pos, layout)
    }

    /// Allocates contiguous pages.
    fn alloc_pages(&self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        self.pballoc.lock().alloc_pages(num_pages, align_pow2)
    }

    /// Gives back the allocated pages starts from `pos` to the page allocator.
    fn dealloc_pages(&self, pos: usize, num_pages: usize) {
        self.pballoc.lock().dealloc_pages(pos, num_pages)
    }

    /// Returns the number of allocated bytes in the byte allocator.
    fn used_bytes(&self) -> usize {
        self.pballoc.lock().used_bytes()
    }

    /// Returns the number of available bytes in the byte allocator.
    fn available_bytes(&self) -> usize {
        self.pballoc.lock().available_bytes()
    }

    /// Returns the number of allocated pages in the page allocator.
    fn used_pages(&self) -> usize {
        self.pballoc.lock().used_pages()
    }

    /// Returns the number of available pages in the page allocator.
    fn available_pages(&self) -> usize {
        self.pballoc.lock().available_pages()
    }
}

unsafe impl GlobalAlloc for AIOAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok(ptr) = <AIOAllocator as GlobalAllocator>::alloc(self, layout) {
            ptr.as_ptr()
        } else {
            alloc::alloc::handle_alloc_error(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        <AIOAllocator as GlobalAllocator>::dealloc(self, NonNull::new(ptr).expect("dealloc null ptr"), layout)
    }
}
