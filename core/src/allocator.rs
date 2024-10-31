//! Tools for using different allocators with QuickJS.

use crate::qjs;
use std::ptr;

mod rust;

pub use rust::RustAllocator;

/// The allocator interface
///
/// # Safety
/// Failure to implement this trait correctly will result in undefined behavior.
/// - `alloc` must return a either a null pointer or a pointer to an available region of memory
/// atleast `size` bytes and aligned to the size of `usize`.
/// - `realloc` must either return a null pointer or return a pointer to an available region of
/// memory atleast `new_size` bytes and aligned to the size of `usize`.
/// - `usable_size` must return the amount of available memory for any allocation allocated with
/// this allocator.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
pub unsafe trait Allocator {
    /// Allocate new memory
    ///
    ///
    fn alloc(&mut self, size: usize) -> *mut u8;

    /// De-allocate previously allocated memory
    ///
    /// # Safety
    /// Caller must ensure that the pointer that is being deallocated was allocated by the same
    /// Allocator instance.
    unsafe fn dealloc(&mut self, ptr: *mut u8);

    /// Re-allocate previously allocated memory
    ///
    /// # Safety
    /// Caller must ensure that the pointer points to an allocation that was allocated by the same
    /// Allocator instance.
    unsafe fn realloc(&mut self, ptr: *mut u8, new_size: usize) -> *mut u8;

    /// Get usable size of allocated memory region
    ///
    /// # Safety
    /// Caller must ensure that the pointer handed to this function points to an allocation
    /// allocated by the same allocator instance.
    unsafe fn usable_size(ptr: *mut u8) -> usize
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

    fn size_t(size: usize) -> qjs::size_t {
        size.try_into().expect(qjs::SIZE_T_ERROR)
    }

    unsafe extern "C" fn malloc<A>(
        state: *mut qjs::JSMallocState,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        if size == 0 {
            return ptr::null_mut();
        }

        let state = &mut *state;

        if state.malloc_size + size > state.malloc_limit {
            return ptr::null_mut();
        }

        let rust_size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        // simulate the default behavior of libc::malloc

        let allocator = &mut *(state.opaque as *mut DynAllocator);

        let res = allocator.alloc(rust_size as _);

        if res.is_null() {
            return ptr::null_mut();
        }

        let size = A::usable_size(res);

        state.malloc_count += 1;
        state.malloc_size += Self::size_t(size);

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

        let size = A::usable_size(ptr as *mut u8);

        let allocator = &mut *(state.opaque as *mut DynAllocator);
        allocator.dealloc(ptr as _);

        state.malloc_size -= Self::size_t(size);
    }

    unsafe extern "C" fn realloc<A>(
        state: *mut qjs::JSMallocState,
        ptr: *mut qjs::c_void,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        let state_ref = &mut *state;
        let allocator = &mut *(state_ref.opaque as *mut DynAllocator);

        // simulate the default behavior of libc::realloc
        if ptr.is_null() {
            return Self::malloc::<A>(state, size);
        } else if size == 0 {
            Self::free::<A>(state, ptr);
            return ptr::null_mut();
        }

        let old_size = Self::size_t(A::usable_size(ptr as *mut u8));

        let new_malloc_size = state_ref.malloc_size - old_size + size;
        if new_malloc_size > state_ref.malloc_limit {
            return ptr::null_mut();
        }

        let ptr = allocator.realloc(ptr as _, size.try_into().expect(qjs::SIZE_T_ERROR))
            as *mut qjs::c_void;

        if ptr.is_null() {
            return ptr::null_mut();
        }

        let actual_size = Self::size_t(A::usable_size(ptr as *mut u8));

        state_ref.malloc_size -= old_size;
        state_ref.malloc_size += actual_size;

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
