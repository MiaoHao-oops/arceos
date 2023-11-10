use core::alloc::Layout;
use core::ptr::NonNull;

use crate::{AllocResult, AllocError, BaseAllocator, PageAllocator, ByteAllocator};

pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    base: usize,
    b_current: usize,
    used_bytes: usize,
    used_objs: usize,
    top: usize,
    p_current: usize,
    used_pages: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    /// Creates a new empty `EarlyAllocator`
    pub const fn new() -> Self {
        Self {
            base: 0,
            b_current: 0,
            used_bytes: 0,
            used_objs: 0,
            top: 0,
            p_current: 0,
            used_pages: 0,
        }
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        assert!(PAGE_SIZE.is_power_of_two());
        let end = super::align_down(start + size, PAGE_SIZE);
        let start = super::align_up(start, PAGE_SIZE);
        self.base = start;
        self.top = end;
        self.b_current = self.base;
        self.p_current = self.top;
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        Err(AllocError::NoMemory) // unsupported
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if align_pow2 % PAGE_SIZE != 0 {
            return Err(AllocError::InvalidParam);
        }
        let align_pow2 = align_pow2 / PAGE_SIZE;
        if !align_pow2.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }
        let pages_ptr = self.top - PAGE_SIZE * num_pages;
        let pages_ptr = super::align_down(pages_ptr, align_pow2 * PAGE_SIZE);
        if pages_ptr >= self.b_current {
            self.p_current = pages_ptr;
            self.used_pages += num_pages;
            Ok(pages_ptr)
        } else {
            Err(AllocError::NoMemory)
        }
    }

    fn dealloc_pages(&mut self, _pos: usize, num_pages: usize) {
        // use a page counter `used_pages`, if all pages are dealloced,
        // free all pages
        self.used_pages -= num_pages;
        if self.used_pages == 0 {
            self.p_current = self.top;
        }
    }

    fn total_pages(&self) -> usize {
        // unsupported
        0
    }

    fn used_pages(&self) -> usize {
        (self.top - self.p_current) / PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        // unsupported
        0
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align_pow2 = layout.align();
        let bytes_ptr = super::align_up(self.b_current, align_pow2);
        let bytes_end = bytes_ptr + layout.size();
        if bytes_end <= self.p_current {
            self.b_current = bytes_end;
            self.used_bytes = self.b_current - self.base;
            self.used_objs += 1;
            Ok(NonNull::new(bytes_ptr as *mut u8).unwrap())
        } else {
            Err(AllocError::NoMemory)
        }
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        self.used_objs -= 1;
        if self.used_objs == 0 {
            self.b_current = self.base;
            self.used_bytes = 0;
        }
    }

    fn total_bytes(&self) -> usize {
        // unsupported
        0
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        // unsupported
        0
    }
}
