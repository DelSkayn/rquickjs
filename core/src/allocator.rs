//! Tools for using different allocators with QuickJS.

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

#[derive(Debug)]
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
            js_malloc: Some(Self::malloc::<A>),
            js_free: Some(Self::free::<A>),
            js_realloc: Some(Self::realloc::<A>),
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

    unsafe extern "C" fn malloc<A>(
        state: *mut qjs::JSMallocState,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        if size == 0 {
            return null_mut();
        }

        let state = &mut *state;

        if state.malloc_size + size > state.malloc_limit {
            return null_mut();
        }

        let rust_size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        // simulate the default behavior of libc::malloc

        let allocator = &mut *(state.opaque as *mut DynAllocator);

        let res = allocator.alloc(rust_size as _);

        if res.is_null() {
            return null_mut();
        }

        let size = A::usable_size(res);

        println!("ALLOC: {}", size);

        state.malloc_count += 1;
        state.malloc_size += qjs::size_t::try_from(size).expect(qjs::SIZE_T_ERROR);

        res as *mut qjs::c_void
    }

    unsafe extern "C" fn free<A>(state: *mut qjs::JSMallocState, ptr: *mut qjs::c_void)
    where
        A: Allocator,
    {
        // simulate the default behavior of libc::free
        if ptr.is_null() {
            // nothing to do
            return;
        }

        let state = &mut *state;
        state.malloc_count -= 1;

        let size = A::usable_size(ptr as RawMemPtr);

        let allocator = &mut *(state.opaque as *mut DynAllocator);
        allocator.dealloc(ptr as _);

        state.malloc_size -= qjs::size_t::try_from(size).expect(qjs::SIZE_T_ERROR);
        println!("FREE: {}", size);
    }

    unsafe extern "C" fn realloc<A>(
        state: *mut qjs::JSMallocState,
        ptr: *mut qjs::c_void,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        let state = &mut *state;
        let allocator = &mut *(state.opaque as *mut DynAllocator);

        // simulate the default behavior of libc::realloc
        if ptr.is_null() {
            if state.malloc_size + size > state.malloc_limit {
                return null_mut();
            }

            let rust_size = size.try_into().expect(qjs::SIZE_T_ERROR);

            // alloc new memory chunk
            let res = allocator.alloc(rust_size);

            if res.is_null() {
                return null_mut();
            }

            state.malloc_count += 1;
            state.malloc_size += size;

            println!("ALLOC: {}", size);

            return res as *mut qjs::c_void;
        } else if size == 0 {
            let old_size = A::usable_size(ptr as RawMemPtr);
            // free memory chunk
            allocator.dealloc(ptr as _);

            state.malloc_count -= 1;
            state.malloc_size -= qjs::size_t::try_from(old_size).expect(qjs::SIZE_T_ERROR);

            println!("FREE: {}", size);

            return null_mut();
        }

        let old_size = A::usable_size(ptr as RawMemPtr);

        let new_malloc_size =
            state.malloc_size - qjs::size_t::try_from(old_size).expect(qjs::SIZE_T_ERROR) + size;

        if new_malloc_size > state.malloc_limit {
            return null_mut();
        }

        let size = size.try_into().expect(qjs::SIZE_T_ERROR);

        let ptr = allocator.realloc(ptr as _, size) as *mut qjs::c_void;

        state.malloc_size = new_malloc_size;

        println!("FREE: {}", old_size);
        println!("ALLOC: {}", size);

        ptr
    }

    unsafe extern "C" fn malloc_usable_size<A>(ptr: *const qjs::c_void) -> qjs::size_t
    where
        A: Allocator,
    {
        // simulate the default behavior of libc::malloc_usable_size
        if ptr.is_null() {
            return 0;
        }
        A::usable_size(ptr as _).try_into().unwrap()
    }
}
