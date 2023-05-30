use std::{ffi::CString, result::Result as StdResult};

use async_lock::Mutex;

#[cfg(feature = "loader")]
use crate::loader::{RawLoader, Resolver};
use crate::{
    allocator::Allocator,
    result::JobException,
    safe_ref::{Ref, Weak},
    Error, Result,
};

use super::{
    raw::{Opaque, RawRuntime},
    MemoryUsage,
};

#[derive(Clone)]
pub struct AsyncWeakRuntime(Weak<Mutex<RawRuntime>>);

#[derive(Clone)]
pub struct AsyncRuntime {
    inner: Ref<Mutex<RawRuntime>>,
}

impl AsyncRuntime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// *If the `"rust-alloc"` feature is enabled the Rust's global allocator will be used in favor of libc's one.*
    pub fn new() -> Result<Self> {
        let opaque = Opaque::new();
        let rt = unsafe { RawRuntime::new(opaque) }.ok_or(Error::Allocation)?;
        Ok(Self {
            inner: Ref::new(Mutex::new(rt)),
        })
    }

    /// Create a new runtime using specified allocator
    ///
    /// Will generally only fail if not enough memory was available.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
    #[cfg(feature = "allocator")]
    pub fn new_with_alloc<A>(allocator: A) -> Result<Self>
    where
        A: Allocator + 'static,
    {
        let opaque = Opaque::new();
        let rt = unsafe { RawRuntime::new_with_allocator(opaque, allocator) }
            .ok_or(Error::Allocation)?;
        Ok(Self {
            inner: Ref::new(Mutex::new(rt)),
        })
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> AsyncWeakRuntime {
        AsyncWeakRuntime(Ref::downgrade(&self.inner))
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    /// If the provided closure returns `true` the interpreter will raise and uncatchable
    /// exception and return control flow to the caller.
    #[inline]
    pub async fn set_interrupt_handler(&self, handler: Option<Box<dyn FnMut() -> bool + 'static>>) {
        unsafe {
            self.inner.lock().await.set_interrupt_handler(handler);
        }
    }

    /// Set the module loader
    #[cfg(feature = "loader")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
    pub async fn set_loader<R, L>(&self, resolver: R, loader: L)
    where
        R: Resolver + 'static,
        L: RawLoader + 'static,
    {
        unsafe {
            self.inner.lock().await.set_loader(resolver, loader);
        }
    }

    /// Set the info of the runtime
    pub async fn set_info<S: Into<Vec<u8>>>(&self, info: S) -> Result<()> {
        let string = CString::new(info)?;
        unsafe {
            self.inner.lock().await.set_info(string);
        }
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    ///
    /// Note that is a Noop when a custom allocator is being used,
    /// as is the case for the "rust-alloc" or "allocator" features.
    pub async fn set_memory_limit(&self, limit: usize) {
        unsafe {
            self.inner.lock().await.set_memory_limit(limit);
        }
    }

    /// Set a limit on the max size of stack the runtime will use.
    ///
    /// The default values is 256x1024 bytes.
    pub async fn set_max_stack_size(&self, limit: usize) {
        unsafe {
            self.inner.lock().await.set_max_stack_size(limit);
        }
    }

    /// Set a memory threshold for garbage collection.
    pub async fn set_gc_threshold(&self, threshold: usize) {
        unsafe {
            self.inner.lock().await.set_gc_threshold(threshold);
        }
    }

    /// Manually run the garbage collection.
    ///
    /// Most of quickjs values are reference counted and
    /// will automaticly free themselfs when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub async fn run_gc(&self) {
        unsafe {
            self.inner.lock().await.run_gc();
        }
    }

    /// Get memory usage stats
    pub async fn memory_usage(&self) -> MemoryUsage {
        unsafe { self.inner.lock().await.memory_usage() }
    }

    /// Test for pending jobs
    ///
    /// Returns true when at least one job is pending.
    #[inline]
    pub async fn is_job_pending(&self) -> bool {
        self.inner.lock().await.is_job_pending()
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    #[inline]
    pub async fn execute_pending_job(&self) -> StdResult<bool, JobException> {
        self.inner
            .lock()
            .await
            .execute_pending_job()
            .map_err(|_e| todo!())
    }
}
