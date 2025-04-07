//! QuickJS runtime related types.

use super::{opaque::Opaque, raw::RawRuntime, InterruptHandler, MemoryUsage, RejectionTracker};
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{Loader, Resolver};
use crate::{result::JobException, Context, Mut, Ref, Result, Weak};
use std::{ffi::CString, ptr::NonNull, result::Result as StdResult};

/// A weak handle to the runtime.
///
/// Holding onto this struct does not prevent the runtime from being dropped.
#[derive(Clone)]
#[repr(transparent)]
pub struct WeakRuntime(Weak<Mut<RawRuntime>>);

impl WeakRuntime {
    pub fn try_ref(&self) -> Option<Runtime> {
        self.0.upgrade().map(|inner| Runtime { inner })
    }
}

/// QuickJS runtime, entry point of the library.
#[derive(Clone)]
#[repr(transparent)]
pub struct Runtime {
    pub(crate) inner: Ref<Mut<RawRuntime>>,
}

impl Runtime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// *If the `"rust-alloc"` feature is enabled the Rust's global allocator will be used in favor of libc's one.*
    pub fn new() -> Result<Self> {
        let opaque = Opaque::new();
        let rt = unsafe { RawRuntime::new(opaque)? };
        Ok(Self {
            inner: Ref::new(Mut::new(rt)),
        })
    }

    /// Create a new runtime using specified allocator
    ///
    /// Will generally only fail if not enough memory was available.
    pub fn new_with_alloc<A>(allocator: A) -> Result<Self>
    where
        A: Allocator + 'static,
    {
        let opaque = Opaque::new();
        let rt = unsafe { RawRuntime::new_with_allocator(opaque, allocator)? };
        Ok(Self {
            inner: Ref::new(Mut::new(rt)),
        })
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> WeakRuntime {
        WeakRuntime(Ref::downgrade(&self.inner))
    }

    /// Set a closure which is called when a Promise is rejected.
    #[inline]
    pub fn set_host_promise_rejection_tracker(&self, tracker: Option<RejectionTracker>) {
        unsafe {
            self.inner
                .lock()
                .set_host_promise_rejection_tracker(tracker);
        }
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    /// If the provided closure returns `true` the interpreter will raise and uncatchable
    /// exception and return control flow to the caller.
    #[inline]
    pub fn set_interrupt_handler(&self, handler: Option<InterruptHandler>) {
        unsafe {
            self.inner.lock().set_interrupt_handler(handler);
        }
    }

    /// Set the module loader
    #[cfg(feature = "loader")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
    pub fn set_loader<R, L>(&self, resolver: R, loader: L)
    where
        R: Resolver + 'static,
        L: Loader + 'static,
    {
        unsafe {
            self.inner.lock().set_loader(resolver, loader);
        }
    }

    /// Set the info of the runtime
    pub fn set_info<S: Into<Vec<u8>>>(&self, info: S) -> Result<()> {
        let string = CString::new(info)?;
        unsafe {
            self.inner.lock().set_info(string);
        }
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    ///
    /// Note that is a Noop when a custom allocator is being used,
    /// as is the case for the "rust-alloc" or "allocator" features.
    pub fn set_memory_limit(&self, limit: usize) {
        unsafe {
            self.inner.lock().set_memory_limit(limit);
        }
    }

    /// Set a limit on the max size of stack the runtime will use.
    ///
    /// The default values is 256x1024 bytes.
    pub fn set_max_stack_size(&self, limit: usize) {
        unsafe {
            self.inner.lock().set_max_stack_size(limit);
        }
    }

    /// Set a memory threshold for garbage collection.
    pub fn set_gc_threshold(&self, threshold: usize) {
        unsafe {
            self.inner.lock().set_gc_threshold(threshold);
        }
    }

    /// Set debug flags for dumping memory
    pub fn set_dump_flags(&self, flags: u64) {
        unsafe {
            self.inner.lock().set_dump_flags(flags);
        }
    }

    /// Manually run the garbage collection.
    ///
    /// Most of QuickJS values are reference counted and
    /// will automatically free themselves when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub fn run_gc(&self) {
        unsafe {
            self.inner.lock().run_gc();
        }
    }

    /// Get memory usage stats
    pub fn memory_usage(&self) -> MemoryUsage {
        unsafe { self.inner.lock().memory_usage() }
    }

    /// Test for pending jobs
    ///
    /// Returns true when at least one job is pending.
    #[inline]
    pub fn is_job_pending(&self) -> bool {
        self.inner.lock().is_job_pending()
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    #[inline]
    pub fn execute_pending_job(&self) -> StdResult<bool, JobException> {
        let mut lock = self.inner.lock();
        lock.update_stack_top();
        lock.execute_pending_job().map_err(|e| {
            JobException(unsafe {
                Context::from_raw(
                    NonNull::new(e).expect("QuickJS returned null ptr for job error"),
                    self.clone(),
                )
            })
        })
    }
}

// Since all functions which use runtime are behind a mutex
// sending the runtime to other threads should be fine.
#[cfg(feature = "parallel")]
unsafe impl Send for Runtime {}
#[cfg(feature = "parallel")]
unsafe impl Send for WeakRuntime {}

// Since a global lock needs to be locked for safe use
// using runtime in a sync way should be safe as
// simultaneous accesses is synchronized behind a lock.
#[cfg(feature = "parallel")]
unsafe impl Sync for Runtime {}
#[cfg(feature = "parallel")]
unsafe impl Sync for WeakRuntime {}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn base_runtime() {
        let rt = Runtime::new().unwrap();
        rt.set_info("test runtime").unwrap();
        rt.set_memory_limit(0xFFFF);
        rt.set_gc_threshold(0xFF);
        rt.run_gc();
    }
}
