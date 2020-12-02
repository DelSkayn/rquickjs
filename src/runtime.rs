use crate::{qjs, value, Ctx, Error, Function, RegisteryKey, Result, SafeRef, SafeWeakRef};
use fxhash::FxHashSet as HashSet;
use std::{any::Any, ffi::CString, mem};

pub use qjs::JSMemoryUsage as MemoryUsage;

#[cfg(feature = "allocator")]
use crate::{allocator::AllocatorHolder, Allocator};

#[cfg(feature = "loader")]
use crate::{loader::LoaderHolder, Loader, Resolver};

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use crate::async_shim::JoinHandle;

#[derive(Clone)]
#[repr(transparent)]
pub struct WeakRuntime(SafeWeakRef<Inner>);

impl WeakRuntime {
    pub fn try_ref(&self) -> Option<Runtime> {
        self.0.try_ref().map(|inner| Runtime { inner })
    }
}

/// Opaque book keeping data for rust.
pub struct Opaque {
    /// The registery, used to keep track of which registery values belong to this runtime.
    pub registery: HashSet<RegisteryKey>,

    /// Used to carry a panic if a callback triggered one.
    pub panic: Option<Box<dyn Any + Send + 'static>>,

    /// Used to ref Runtime from Ctx
    pub runtime: WeakRuntime,
}

impl Opaque {
    fn new(runtime: &Runtime) -> Self {
        Opaque {
            registery: HashSet::default(),
            panic: None,
            runtime: runtime.weak(),
        }
    }
}

pub(crate) struct Inner {
    pub(crate) rt: *mut qjs::JSRuntime,
    // To keep rt info alive for the entire duration of the lifetime of rt
    info: Option<CString>,

    #[cfg(feature = "allocator")]
    #[allow(dead_code)]
    allocator: Option<AllocatorHolder>,

    #[cfg(feature = "loader")]
    #[allow(dead_code)]
    loader: Option<LoaderHolder>,
}

/// Quickjs runtime, entry point of the library.
#[derive(Clone)]
#[repr(transparent)]
pub struct Runtime {
    pub(crate) inner: SafeRef<Inner>,
}

impl Runtime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// If the `rust-alloc` feature is enabled the Rust's global allocator will be used in favor of libc's one.
    pub fn new() -> Result<Self> {
        #[cfg(not(feature = "rust-alloc"))]
        {
            Self::new_raw(
                unsafe { qjs::JS_NewRuntime() },
                #[cfg(feature = "allocator")]
                None,
            )
        }
        #[cfg(feature = "rust-alloc")]
        Self::new_with_alloc(crate::allocator::RustAllocator)
    }

    #[cfg(feature = "allocator")]
    /// Create a new runtime using specified allocator
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// This function is only available if the `allocator` feature is enabled.
    pub fn new_with_alloc<A>(allocator: A) -> Result<Self>
    where
        A: Allocator + 'static,
    {
        let allocator = AllocatorHolder::new(allocator);
        let functions = AllocatorHolder::functions::<A>();
        let opaque = allocator.opaque_ptr();

        Self::new_raw(
            unsafe { qjs::JS_NewRuntime2(&functions, opaque as _) },
            Some(allocator),
        )
    }

    #[inline]
    fn new_raw(
        rt: *mut qjs::JSRuntime,
        #[cfg(feature = "allocator")] allocator: Option<AllocatorHolder>,
    ) -> Result<Self> {
        if rt.is_null() {
            return Err(Error::Allocation);
        }
        unsafe { Function::init_raw_rt(rt) };
        let runtime = Runtime {
            inner: SafeRef::new(Inner {
                rt,
                info: None,
                #[cfg(feature = "allocator")]
                allocator,
                #[cfg(feature = "loader")]
                loader: None,
            }),
        };
        let opaque = Opaque::new(&runtime);
        unsafe {
            qjs::JS_SetRuntimeOpaque(rt, Box::into_raw(Box::new(opaque)) as *mut _);
        }
        Ok(runtime)
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> WeakRuntime {
        WeakRuntime(self.inner.weak())
    }

    #[cfg(feature = "loader")]
    /// Set the module loader
    ///
    /// # Features
    /// This function is only availble if the `loader` feature is enabled.
    pub fn set_loader<R, L>(&self, resolver: R, loader: L)
    where
        R: Resolver + 'static,
        L: Loader + 'static,
    {
        let mut guard = self.inner.lock();
        let loader = LoaderHolder::new(resolver, loader);
        loader.set_to_runtime(guard.rt);
        guard.loader = Some(loader);
    }

    /// Set the info of the runtime
    pub fn set_info<S: Into<Vec<u8>>>(&self, info: S) -> Result<()> {
        let mut guard = self.inner.lock();
        let string = CString::new(info)?;
        unsafe { qjs::JS_SetRuntimeInfo(guard.rt, string.as_ptr()) }
        guard.info = Some(string);
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime
    /// will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    pub fn set_memory_limit(&self, limit: usize) {
        let guard = self.inner.lock();
        let limit = limit as qjs::size_t;
        unsafe { qjs::JS_SetMemoryLimit(guard.rt, limit) }
        mem::drop(guard);
    }

    /// Set a memory threshold for garbage collection.
    pub fn set_gc_threshold(&self, threshold: usize) {
        let guard = self.inner.lock();
        let threshold = threshold as qjs::size_t;
        unsafe { qjs::JS_SetGCThreshold(guard.rt, threshold) }
        mem::drop(guard);
    }

    /// Manually run the garbage collection.
    ///
    /// Most of quickjs values are reference counted and
    /// will automaticly free themselfs when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub fn run_gc(&self) {
        let guard = self.inner.lock();
        unsafe { qjs::JS_RunGC(guard.rt) }
        mem::drop(guard);
    }

    /// Get memory usage stats
    pub fn memory_usage(&self) -> MemoryUsage {
        let guard = self.inner.lock();
        let mut stats = mem::MaybeUninit::uninit();
        unsafe {
            qjs::JS_ComputeMemoryUsage(guard.rt, stats.as_mut_ptr());
        }
        mem::drop(guard);
        unsafe { stats.assume_init() }
    }

    /// Test for pending jobs
    ///
    /// Returns true when at least one job is pending.
    pub fn is_job_pending(&self) -> bool {
        let guard = self.inner.lock();
        let res = 0 != unsafe { qjs::JS_IsJobPending(guard.rt) };
        mem::drop(guard);
        res
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    pub fn execute_pending_job(&self) -> Result<bool> {
        let guard = self.inner.lock();
        let mut ctx_ptr = mem::MaybeUninit::<*mut qjs::JSContext>::uninit();
        let result = unsafe { qjs::JS_ExecutePendingJob(guard.rt, ctx_ptr.as_mut_ptr()) };
        if result == 0 {
            // no jobs executed
            return Ok(false);
        }
        let ctx_ptr = unsafe { ctx_ptr.assume_init() };
        if result == 1 {
            // single job executed
            return Ok(true);
        }
        // exception thrown
        let ctx = Ctx::from_ptr(ctx_ptr);
        let res = Err(unsafe { value::get_exception(ctx) });
        mem::drop(guard);
        res
    }

    #[cfg(any(feature = "tokio", feature = "async-std"))]
    /// Execute pending jobs using async runtime
    ///
    /// When `max_idle_cycles` is `Some(N)` execution will be stopped if no pending jobs still in queue while N polling cycles.
    /// When `max_idle_cycles` is `None` execution will not been stopped until runtime is dropped. All newly added pending tasks will be executed as well.
    ///
    /// # Features
    /// Either __tokio__ or __async-std__ runtime is supported depending from used cargo feature.
    pub fn spawn_pending_jobs(&self, max_idle_cycles: Option<usize>) -> JoinHandle<()> {
        use crate::async_shim::{spawn_parallel, yield_now};

        let rt = self.weak();
        spawn_parallel(async move {
            let mut idle_cycles = 0;
            'run: while let Some(rt) = rt.try_ref() {
                loop {
                    match rt.execute_pending_job() {
                        Ok(false) => {
                            // queue was empty
                            idle_cycles += 1;
                            break;
                        }
                        result => {
                            if let Err(error) = result {
                                eprintln!("Error when pending job executing: {}", error);
                            }
                            idle_cycles = 0;
                            // task was executed
                            yield_now().await;
                        }
                    }
                }
                // queue was empty
                if let Some(max_idle_cycles) = max_idle_cycles {
                    if idle_cycles >= max_idle_cycles {
                        break 'run;
                    }
                }
                yield_now().await;
            }
        })
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let ptr = qjs::JS_GetRuntimeOpaque(self.rt);
            let _opaque: Box<Opaque> = Box::from_raw(ptr as *mut _);
            qjs::JS_FreeRuntime(self.rt)
        }
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
// simultanious accesses is syncronized behind a lock.
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
