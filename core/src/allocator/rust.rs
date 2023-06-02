use std::{
    alloc::{alloc, dealloc, realloc, Layout},
    mem::size_of,
    ptr::null_mut,
};

use super::{Allocator, RawMemPtr};

/// The largest value quickjs will allocate is a u64;
/// So all allocated memory must have the same aligment is this largest size.
const ALLOC_ALIGN: usize = std::mem::align_of::<u64>();

#[derive(Copy, Clone)]
#[repr(transparent)]
struct Header {
    size: usize,
}

/// Head needs to be atleast alloc alligned so all that values after the header are aligned.
const HEADER_SIZE: usize = size_of::<Header>().max(ALLOC_ALIGN);
const HEADER_OFFSET: isize = HEADER_SIZE as _;

#[inline]
fn round_size(size: usize) -> usize {
    // this will be optimized by the compiler
    // to something like (size + <off>) & <mask>
    (size + ALLOC_ALIGN - 1) / ALLOC_ALIGN * ALLOC_ALIGN
}

/// The allocator which uses Rust global allocator
pub struct RustAllocator;

impl Allocator for RustAllocator {
    fn alloc(&mut self, size: usize) -> RawMemPtr {
        let size = round_size(size);
        let alloc_size = size + HEADER_SIZE;
        let layout = if let Ok(layout) = Layout::from_size_align(alloc_size, ALLOC_ALIGN) {
            layout
        } else {
            return null_mut();
        };

        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            return null_mut();
        }
        {
            let header = unsafe { &mut *(ptr as *mut Header) };
            header.size = size;
        }

        unsafe { ptr.offset(HEADER_OFFSET) }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn dealloc(&mut self, ptr: RawMemPtr) {
        let ptr = unsafe { ptr.sub(HEADER_OFFSET) };
        let alloc_size = {
            let header = unsafe { &*(ptr as *const Header) };
            header.size + HEADER_SIZE
        };
        let layout = unsafe { Layout::from_size_align_unchecked(alloc_size, ALLOC_ALIGN) };

        unsafe { dealloc(ptr, layout) };
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn realloc(&mut self, ptr: RawMemPtr, new_size: usize) -> RawMemPtr {
        let new_size = round_size(new_size);
        let ptr = unsafe { ptr.sub(HEADER_OFFSET) };
        let alloc_size = {
            let header = unsafe { &*(ptr as *const Header) };
            header.size + HEADER_SIZE
        };
        let layout = unsafe { Layout::from_size_align_unchecked(alloc_size, ALLOC_ALIGN) };

        let new_alloc_size = new_size + HEADER_SIZE;

        let ptr = unsafe { realloc(ptr, layout, new_alloc_size) };

        if ptr.is_null() {
            return null_mut();
        }
        {
            let header = unsafe { &mut *(ptr as *mut Header) };
            header.size = new_size;
        }

        unsafe { ptr.offset(HEADER_OFFSET) }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn usable_size(ptr: RawMemPtr) -> usize {
        let ptr = unsafe { ptr.sub(HEADER_OFFSET) };
        let header = unsafe { &*(ptr as *const Header) };
        header.size
    }
}
