//! Tools for using different allocators with QuickJS.

use std::ptr::NonNull;

use crate::qjs;

mod rust;

pub use rust::RustAllocator;

/// The allocator interface
///
/// # Safety
/// Failure to implement this trait correctly will result in undefined behavior.
/// - `alloc` must return a either a null pointer or a pointer to an available region of memory
///   atleast `size` bytes and aligned to the size of `usize`.
/// - `realloc` must either return a null pointer or return a pointer to an available region of
///   memory atleast `new_size` bytes and aligned to the size of `usize`.
/// - `usable_size` must return the amount of available memory for any allocation allocated with
///   this allocator.
pub unsafe trait Allocator {
    /// Allocate new memory
    ///
    ///
    fn alloc(&mut self, size: usize) -> *mut u8;

    /// Allocates memory for an array of num objects of size and initializes all bytes in the allocated storage to zero.
    ///
    ///
    fn calloc(&mut self, count: usize, size: usize) -> *mut u8;

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

#[derive(Debug)]
pub(crate) struct AllocatorHolder {
    ptr: NonNull<()>,
    drop: unsafe fn(NonNull<()>),
}

impl Drop for AllocatorHolder {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(self.ptr);
        }
    }
}

#[allow(clippy::extra_unused_type_parameters)]
impl AllocatorHolder {
    pub(crate) fn functions<A>() -> qjs::JSMallocFunctions
    where
        A: Allocator,
    {
        qjs::JSMallocFunctions {
            js_calloc: Some(Self::calloc::<A>),
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
        let alloc = Box::new(allocator);
        //Box::into_raw is gaurenteed to return a non-null pointer.
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(alloc)) };
        let drop = Self::drop::<A>;
        AllocatorHolder {
            ptr: ptr.cast(),
            drop,
        }
    }

    pub(crate) fn opaque_ptr(&self) -> *mut () {
        self.ptr.as_ptr()
    }

    unsafe fn drop<A>(ptr: NonNull<()>) {
        let _ = Box::<A>::from_raw(ptr.cast().as_ptr());
    }

    unsafe extern "C" fn calloc<A>(
        opaque: *mut qjs::c_void,
        count: qjs::size_t,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        let allocator = &mut *(opaque as *mut A);
        let rust_size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        let rust_count: usize = count.try_into().expect(qjs::SIZE_T_ERROR);
        allocator.calloc(rust_count, rust_size) as *mut qjs::c_void
    }

    unsafe extern "C" fn malloc<A>(opaque: *mut qjs::c_void, size: qjs::size_t) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        let allocator = &mut *(opaque as *mut A);
        let rust_size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        allocator.alloc(rust_size) as *mut qjs::c_void
    }

    unsafe extern "C" fn free<A>(opaque: *mut qjs::c_void, ptr: *mut qjs::c_void)
    where
        A: Allocator,
    {
        // simulate the default behavior of libc::free
        if ptr.is_null() {
            // nothing to do
            return;
        }

        let allocator = &mut *(opaque as *mut A);
        allocator.dealloc(ptr as _);
    }

    unsafe extern "C" fn realloc<A>(
        opaque: *mut qjs::c_void,
        ptr: *mut qjs::c_void,
        size: qjs::size_t,
    ) -> *mut qjs::c_void
    where
        A: Allocator,
    {
        let rust_size: usize = size.try_into().expect(qjs::SIZE_T_ERROR);
        let allocator = &mut *(opaque as *mut A);
        allocator.realloc(ptr as _, rust_size) as *mut qjs::c_void
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
