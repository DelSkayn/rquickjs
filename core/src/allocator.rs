//! Tools for using different allocators with quickjs.

use crate::qjs;
use std::{convert::TryInto, ptr::null_mut};

mod rust;
pub use rust::RustAllocator;

/// Raw memory pointer
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
pub type RawMemPtr = *mut u8;

/// The allocator interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
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

pub(crate) struct AllocatorHolder(*mut DynAllocator);

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
        let size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
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
        let size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        let state = &*state;
        let allocator = &mut *(state.opaque as *mut DynAllocator);

        // simulate the default behavior of libc::realloc
        if ptr.is_null() {
            // alloc new memory chunk
            return allocator.alloc(size) as _;
        } else if size == 0 {
            // free memory chunk
            allocator.dealloc(ptr as _);
            return null_mut();
        }

        allocator.realloc(ptr as _, size) as _
    }

    unsafe extern "C" fn malloc_usable_size<A>(ptr: *const qjs::c_void) -> qjs::size_t
    where
        A: Allocator,
    {
        // simulate the default bahavior of libc::malloc_usable_size
        if ptr.is_null() {
            return 0;
        }
        A::usable_size(ptr as _).try_into().unwrap()
    }
}
