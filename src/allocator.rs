use crate::qjs;
use std::{
    alloc::{alloc, dealloc, realloc, Layout},
    mem::size_of,
    ptr::null_mut,
};

/// Raw memory pointer
pub type RawMemPtr = *mut u8;

/// The allocator trait
///
/// # Features
/// This trait is only available if the `allocator` feature is enabled.
pub trait Allocator {
    /// Allocate new memory
    fn alloc(&mut self, size: usize) -> RawMemPtr;

    /// De-allocate previously allocated memory
    fn dealloc(&mut self, ptr: RawMemPtr);

    /// Re-allocate previously allocated memory
    fn realloc(&mut self, ptr: RawMemPtr, new_size: usize) -> RawMemPtr;

    /// Get usable size of allocated memory region
    fn usable_size(ptr: RawMemPtr) -> usize
    where
        Self: Sized;
}

type DynAllocator = Box<dyn Allocator>;

pub struct AllocatorHolder(*mut DynAllocator);

impl Drop for AllocatorHolder {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.0) };
    }
}

impl AllocatorHolder {
    pub(crate) fn functions<A>() -> qjs::JSMallocFunctions
    where
        A: Allocator,
    {
        qjs::JSMallocFunctions {
            js_malloc: Some(Self::malloc),
            js_free: Some(Self::free),
            js_realloc: Some(Self::realloc),
            js_malloc_usable_size: Some(Self::malloc_usable_size::<A>),
        }
    }

    pub(crate) fn new<A>(allocator: A) -> Self
    where
        A: Allocator + 'static,
    {
        Self(Box::into_raw(Box::new(Box::new(allocator))))
    }

    pub(crate) fn opaque_ptr(&self) -> *mut DynAllocator {
        self.0
    }

    unsafe extern "C" fn malloc(
        state: *mut qjs::JSMallocState,
        size: qjs::size_t,
    ) -> *mut qjs::c_void {
        // simulate the default behavior of libc::malloc
        if size == 0 {
            return null_mut();
        }

        let state = &*state;
        let allocator = &mut *(state.opaque as *mut DynAllocator);

        allocator.alloc(size as _) as _
    }

    unsafe extern "C" fn free(state: *mut qjs::JSMallocState, ptr: *mut qjs::c_void) {
        // simulate the default behavior of libc::free
        if ptr.is_null() {
            // nothing to do
            return;
        }

        let state = &*state;
        let allocator = &mut *(state.opaque as *mut DynAllocator);

        allocator.dealloc(ptr as _);
    }

    unsafe extern "C" fn realloc(
        state: *mut qjs::JSMallocState,
        ptr: *mut qjs::c_void,
        size: qjs::size_t,
    ) -> *mut qjs::c_void {
        let state = &*state;
        let allocator = &mut *(state.opaque as *mut DynAllocator);

        // simulate the default behavior of libc::realloc
        if ptr.is_null() {
            // alloc new memory chunk
            return allocator.alloc(size as _) as _;
        } else if size == 0 {
            // free memory chunk
            allocator.dealloc(ptr as _);
            return null_mut();
        }

        allocator.realloc(ptr as _, size as _) as _
    }

    unsafe extern "C" fn malloc_usable_size<A>(ptr: *const qjs::c_void) -> qjs::size_t
    where
        A: Allocator,
    {
        // simulate the default bahavior of libc::malloc_usable_size
        if ptr.is_null() {
            return 0;
        }
        A::usable_size(ptr as _) as _
    }
}

#[cfg(target_pointer_width = "32")]
const ALLOC_ALIGN: usize = 4;

#[cfg(target_pointer_width = "64")]
const ALLOC_ALIGN: usize = 8;

#[derive(Copy, Clone)]
#[repr(transparent)]
struct Header {
    size: usize,
}

const HEADER_SIZE: usize = size_of::<Header>();
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

    fn dealloc(&mut self, ptr: RawMemPtr) {
        let ptr = unsafe { ptr.offset(-HEADER_OFFSET) };
        let alloc_size = {
            let header = unsafe { &*(ptr as *const Header) };
            header.size + HEADER_SIZE
        };
        let layout = unsafe { Layout::from_size_align_unchecked(alloc_size, ALLOC_ALIGN) };

        unsafe { dealloc(ptr, layout) };
    }

    fn realloc(&mut self, ptr: RawMemPtr, new_size: usize) -> RawMemPtr {
        let new_size = round_size(new_size);
        let ptr = unsafe { ptr.offset(-HEADER_OFFSET) };
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

    fn usable_size(ptr: RawMemPtr) -> usize {
        let ptr = unsafe { ptr.offset(-HEADER_OFFSET) };
        let header = unsafe { &*(ptr as *const Header) };
        header.size
    }
}
