use crate::{get_exception, qjs, Ctx, Error, Function, Mut, Ref, Result, Weak};

#[cfg(feature = "futures")]
use crate::SendWhenParallel;
#[cfg(feature = "futures")]
use std::{future::Future, pin::Pin};

#[cfg(feature = "registery")]
use crate::RegisteryKey;
#[cfg(feature = "registery")]
use fxhash::FxHashSet as HashSet;

pub use qjs::JSMemoryUsage as MemoryUsage;

#[cfg(feature = "allocator")]
use crate::{allocator::AllocatorHolder, Allocator};

#[cfg(feature = "loader")]
use crate::{loader::LoaderHolder, Loader, Resolver};

#[derive(Clone)]
#[repr(transparent)]
pub struct WeakRuntime(SafeWeakRef<Inner>);

impl WeakRuntime {
    pub fn try_ref(&self) -> Option<Runtime> {
        self.0.try_ref().map(|inner| Runtime {
            inner,
            marker: PhantomData,
        })
    }
}

/// Opaque book keeping data for rust.
pub struct Opaque {
    #[cfg(feature = "registery")]
    /// The registery, used to keep track of which registery values belong to this runtime.
    pub registery: HashSet<RegisteryKey>,

    /// Used to carry a panic if a callback triggered one.
    pub panic: Option<Box<dyn Any + Send + 'static>>,

    /// Used to ref Runtime from Ctx
    pub runtime: WeakRuntime,

    /// Async runtime
    #[cfg(feature = "futures")]
    pub spawner: Box<dyn AsyncSpawner>,
}

impl Opaque {
    fn new(runtime: &Runtime) -> Self {
        Opaque {
            #[cfg(feature = "registery")]
            registery: HashSet::default(),
            panic: None,
            runtime: runtime.weak(),
            #[cfg(feature = "futures")]
            spawner: Box::new(()),
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

#[cfg(feature = "futures")]
impl Inner {
    pub(crate) unsafe fn get_opaque(&mut self) -> &mut Opaque {
        &mut *(qjs::JS_GetRuntimeOpaque(self.rt) as *mut _)
    }
}

/// Quickjs runtime, entry point of the library.
#[derive(Clone)]
#[repr(transparent)]
pub struct Runtime<A = ()> {
    pub(crate) inner: SafeRef<Inner>,
    marker: PhantomData<A>,
}

impl Runtime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// *If the `"rust-alloc"` feature is enabled the Rust's global allocator will be used in favor of libc's one.*
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
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
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

    pub(crate) unsafe fn init_raw(rt: *mut qjs::JSRuntime) {
        Function::init_raw(rt);
    }

    #[inline]
    fn new_raw(
        rt: *mut qjs::JSRuntime,
        #[cfg(feature = "allocator")] allocator: Option<AllocatorHolder>,
    ) -> Result<Self> {
        if rt.is_null() {
            return Err(Error::Allocation);
        }

        unsafe { Self::init_raw(rt) };

        let runtime = Runtime {
            inner: SafeRef::new(Inner {
                rt,
                info: None,
                #[cfg(feature = "allocator")]
                allocator,
                #[cfg(feature = "loader")]
                loader: None,
            }),
            marker: PhantomData,
        };
        let opaque = Opaque::new(&runtime);
        unsafe {
            qjs::JS_SetRuntimeOpaque(rt, Box::into_raw(Box::new(opaque)) as *mut _);
        }
        Ok(runtime)
    }
}

impl<A> Runtime<A> {
    pub(crate) fn as_generic(&self) -> &Runtime {
        unsafe { &*(self as *const _ as *const Runtime) }
    }

    /*pub(crate) fn into_generic(self) -> Runtime {
        Runtime {
            inner: self.inner,
            marker: PhantomData,
        }
    }*/

    /// Get weak ref to runtime
    pub fn weak(&self) -> WeakRuntime {
        WeakRuntime(self.inner.weak())
    }

    /// Set the module loader
    #[cfg(feature = "loader")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
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
        unsafe { qjs::JS_SetRuntimeInfo(guard.rt, string.as_ptr()) };
        guard.info = Some(string);
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    pub fn set_memory_limit(&self, limit: usize) {
        let guard = self.inner.lock();
        unsafe { qjs::JS_SetMemoryLimit(guard.rt, limit as _) };
        mem::drop(guard);
    }

    /// Set a limit on the max size of stack the runtime will use.
    ///
    /// The default values is 256x1024 bytes.
    pub fn set_max_stack_size(&self, limit: usize) {
        let guard = self.inner.lock();
        unsafe { qjs::JS_SetMaxStackSize(guard.rt, limit as _) };
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }

    /// Set a memory threshold for garbage collection.
    pub fn set_gc_threshold(&self, threshold: usize) {
        let guard = self.inner.lock();
        unsafe { qjs::JS_SetGCThreshold(guard.rt, threshold as _) };
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
        unsafe { qjs::JS_RunGC(guard.rt) };
        mem::drop(guard);
    }

    /// Get memory usage stats
    pub fn memory_usage(&self) -> MemoryUsage {
        let guard = self.inner.lock();
        let mut stats = mem::MaybeUninit::uninit();
        unsafe { qjs::JS_ComputeMemoryUsage(guard.rt, stats.as_mut_ptr()) };
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
        let res = Err(unsafe { get_exception(ctx) });
        mem::drop(guard);
        res
    }

    /// Execute pending jobs in blocking mode without yielding control
    ///
    /// When `max_idle_cycles` is `Some(N)` execution will be stopped if no pending jobs still in queue while N polling cycles.
    /// When `max_idle_cycles` is `None` execution will not been stopped until runtime is dropped. All newly added pending tasks will be executed as well.
    ///
    pub fn execute_pending_jobs(&self, max_idle_cycles: Option<usize>) {
        fn yield_now() {}
        self.execute_pending_jobs_sync(max_idle_cycles, yield_now);
    }

    /// Execute pending jobs in blocking mode with yielding control after job was executed
    ///
    /// When `max_idle_cycles` is `Some(N)` execution will be stopped if no pending jobs still in queue while N polling cycles.
    /// When `max_idle_cycles` is `None` execution will not been stopped until runtime is dropped. All newly added pending tasks will be executed as well.
    ///
    pub fn execute_pending_jobs_sync<Y>(&self, max_idle_cycles: Option<usize>, yield_now: Y)
    where
        Y: Fn(),
    {
        let rt = self.weak();

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
                        yield_now();
                    }
                }
            }
            // queue was empty
            if let Some(max_idle_cycles) = max_idle_cycles {
                if idle_cycles >= max_idle_cycles {
                    break 'run;
                }
            }
            yield_now();
        }
    }

    /// Execute pending jobs using async runtime
    ///
    /// When `max_idle_cycles` is `Some(N)` execution will be stopped if no pending jobs still in queue while N polling cycles.
    /// When `max_idle_cycles` is `None` execution will not been stopped until runtime is dropped. All newly added pending tasks will be executed as well.
    ///
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub async fn execute_pending_jobs_async<Y, F>(
        &self,
        max_idle_cycles: Option<usize>,
        yield_now: Y,
    ) where
        Y: Fn() -> F,
        F: Future<Output = ()>,
    {
        let rt = self.weak();

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
    }
}

#[cfg(feature = "futures")]
impl Runtime {
    /// Configure async runtime
    ///
    /// Must be used to get ability to deal with `Promise`s
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn into_async<A>(self, async_rt: A) -> Runtime<A>
    where
        A: AsyncSpawner + 'static,
    {
        {
            let mut inner = self.inner.lock();
            let opaque = unsafe { inner.get_opaque() };
            opaque.spawner = Box::new(async_rt);
        }
        Runtime {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "futures")]
impl<A> Runtime<A> {
    /// Spawn future using configured async runtime
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn_async<F, T>(&self, future: F)
    where
        F: Future<Output = T> + SendWhenParallel + 'static,
        T: SendWhenParallel + 'static,
    {
        let mut inner = self.inner.lock();
        let opaque = unsafe { inner.get_opaque() };
        opaque.spawner.spawn_async(Box::pin(async move {
            future.await;
        }));
    }

    /// Spawn execution pending jobs using async runtime
    ///
    /// When `max_idle_cycles` is `Some(N)` execution will be stopped if no pending jobs still in queue while N polling cycles.
    /// When `max_idle_cycles` is `None` execution will not been stopped until runtime is dropped. All newly added pending tasks will be executed as well.
    ///
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn_pending_jobs(&self, max_idle_cycles: Option<usize>) -> A::JoinHandle
    where
        A: PendingJobsSpawner,
    {
        A::spawn_pending_jobs(self.as_generic(), max_idle_cycles)
    }
}

/// The trait to spawn futures on async runtime
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub trait AsyncSpawner {
    /// Spawn boxed dyn future
    #[cfg(not(feature = "parallel"))]
    fn spawn_async(&self, future: Pin<Box<dyn Future<Output = ()>>>);

    /// Spawn boxed dyn future
    #[cfg(feature = "parallel")]
    fn spawn_async(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>);
}

#[cfg(feature = "futures")]
impl AsyncSpawner for () {
    #[cfg(not(feature = "parallel"))]
    fn spawn_async(&self, _future: Pin<Box<dyn Future<Output = ()>>>) {
        panic!("The async runtime does not configured properly. The `Runtime::into_async()` must be used with a proper async runtime.");
    }

    #[cfg(feature = "parallel")]
    fn spawn_async(&self, _future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        panic!("The async runtime does not configured properly. The `Runtime::into_async()` must be used with a proper async runtime.");
    }
}

/// The trait to spawn execution of pending jobs on async runtime
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub trait PendingJobsSpawner: Sized {
    /// The type of join handle which returns `()`
    type JoinHandle;

    /// Spawn pending jobs using async runtime spawn function
    ///
    /// Usually implemented by calling [`Runtime::execute_pending_jobs_async`] in spawned task.
    fn spawn_pending_jobs(rt: &Runtime, max_idle_cycles: Option<usize>) -> Self::JoinHandle;
}

macro_rules! async_rt_impl {
    ($($(#[$meta:meta])* $type:ident { $join_handle:ty, $spawn_local:path, $spawn:path, $yield_now:path })*) => {
        $(
            $(#[$meta])*
            impl AsyncSpawner for crate::$type {
                #[cfg(not(feature = "parallel"))]
                fn spawn_async(&self, future: Pin<Box<dyn Future<Output = ()>>>)
                {
                    $spawn_local(future);
                }

                #[cfg(feature = "parallel")]
                fn spawn_async(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>)
                {
                    $spawn(future);
                }
            }

            $(#[$meta])*
            impl PendingJobsSpawner for crate::$type {
                type JoinHandle = $join_handle;

                fn spawn_pending_jobs(
                    rt: &Runtime,
                    max_idle_cycles: Option<usize>,
                ) -> Self::JoinHandle {
                    #[cfg(not(feature = "parallel"))]
                    use $spawn_local as spawn_parallel;
                    #[cfg(feature = "parallel")]
                    use $spawn as spawn_parallel;

                    let rt = rt.clone();
                    spawn_parallel(async move {
                        rt.execute_pending_jobs_async(max_idle_cycles, $yield_now).await;
                    })
                }
            }
        )*
    };
}

async_rt_impl! {
    #[cfg(feature = "tokio")]
    Tokio { tokio::task::JoinHandle<()>, tokio::task::spawn_local, tokio::task::spawn, tokio::task::yield_now }
    #[cfg(feature = "async-std")]
    AsyncStd { async_std::task::JoinHandle<()>, async_std::task::spawn_local, async_std::task::spawn, async_std::task::yield_now }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let ptr = qjs::JS_GetRuntimeOpaque(self.rt);
            let opaque: Box<Opaque> = Box::from_raw(ptr as *mut _);
            mem::drop(opaque);
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
