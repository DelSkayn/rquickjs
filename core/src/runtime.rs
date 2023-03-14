use crate::{get_exception, qjs, Ctx, Error, Function, Mut, Ref, Result, Weak, Value};
use std::{any::Any, ffi::CString, mem, panic, collections::HashSet, hash::{Hash, Hasher}};

#[cfg(feature = "futures")]
mod async_runtime;
#[cfg(feature = "futures")]
pub use async_runtime::*;
use rquickjs_sys::{JSContext, JSValue};
#[cfg(feature = "futures")]
mod async_executor;
#[cfg(feature = "futures")]
pub use self::async_executor::*;

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
pub struct WeakRuntime(Weak<Mut<Inner>>);

impl WeakRuntime {
    pub fn try_ref(&self) -> Option<Runtime> {
        self.0.upgrade().map(|inner| Runtime { inner })
    }
}

pub struct UncaughtPromiseReject{
    promise: Value<'static>,
    reason: Value<'static>
}

impl Eq for UncaughtPromiseReject {}

impl PartialEq for UncaughtPromiseReject {
    fn eq(&self, other: &UncaughtPromiseReject) -> bool {
        unsafe{ self.promise.get_ptr() == other.promise.get_ptr() }
    }
}

impl Hash for UncaughtPromiseReject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { self.promise.get_ptr() }.hash(state);
    }
}

type InteruptHandler = Option<Box<dyn FnMut()-> bool + 'static>>;
type UncaughtPromiseRejectionHandler = Option<Box<dyn Fn(&Value) + 'static>>;

/// Opaque book keeping data for rust.
pub struct Opaque {
    /// Used to carry a panic if a callback triggered one.
    pub panic: Option<Box<dyn Any + Send + 'static>>,

    /// The user provided interrupt handler, if any.
    pub interrupt_handler: InteruptHandler,

    /// The user provided promise rejection handler
    pub uncaught_promise_rejection_handler: UncaughtPromiseRejectionHandler,

    /// Used to ref Runtime from Ctx
    pub runtime: WeakRuntime,

    #[cfg(feature = "registery")]
    /// The registery, used to keep track of which registery values belong to this runtime.
    pub registery: HashSet<RegisteryKey>,

    /// Async spawner
    #[cfg(feature = "futures")]
    pub spawner: Option<Spawner>,

    pub uncaught_promise_rejections: HashSet<UncaughtPromiseReject>
}

impl Opaque {
    fn new(runtime: &Runtime) -> Self {
        Opaque {
            panic: None,
            interrupt_handler: None,
            runtime: runtime.weak(),
            #[cfg(feature = "registery")]
            registery: HashSet::default(),
            #[cfg(feature = "futures")]
            spawner: Default::default(),
            uncaught_promise_rejections: HashSet::default(),
            uncaught_promise_rejection_handler: None
        }
    }
}

pub(crate) struct Inner {
    pub(crate) rt: *mut qjs::JSRuntime,

    // To keep rt info alive for the entire duration of the lifetime of rt
    #[allow(dead_code)]
    info: Option<CString>,

    #[cfg(feature = "allocator")]
    #[allow(dead_code)]
    allocator: Option<AllocatorHolder>,

    #[cfg(feature = "loader")]
    #[allow(dead_code)]
    loader: Option<LoaderHolder>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let ptr = qjs::JS_GetRuntimeOpaque(self.rt);
            let mut opaque: Box<Opaque> = Box::from_raw(ptr as *mut _);

            let handler = opaque.uncaught_promise_rejection_handler.take();
            
            for rejection in &opaque.uncaught_promise_rejections {
                if let Some(handler) = &handler {
                    handler.clone()(&rejection.reason)
                }
            }

            mem::drop(opaque);
            qjs::JS_FreeRuntime(self.rt)
        }
    }
}

impl Inner {
    pub(crate) fn update_stack_top(&self) {
        #[cfg(feature = "parallel")]
        unsafe {
            qjs::JS_UpdateStackTop(self.rt);
        }
    }

    #[cfg(feature = "futures")]
    pub(crate) unsafe fn get_opaque(&self) -> &Opaque {
        &*(qjs::JS_GetRuntimeOpaque(self.rt) as *const _)
    }

    pub(crate) unsafe fn get_opaque_mut(&mut self) -> &mut Opaque {
        &mut *(qjs::JS_GetRuntimeOpaque(self.rt) as *mut _)
    }

    pub(crate) fn is_job_pending(&self) -> bool {
        0 != unsafe { qjs::JS_IsJobPending(self.rt) }
    }

    pub(crate) fn execute_pending_job(&mut self) -> Result<bool> {
        let mut ctx_ptr = mem::MaybeUninit::<*mut qjs::JSContext>::uninit();
        self.update_stack_top();
        let result = unsafe { qjs::JS_ExecutePendingJob(self.rt, ctx_ptr.as_mut_ptr()) };
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
        Err(unsafe { get_exception(ctx) })
    }
}

/// Quickjs runtime, entry point of the library.
#[derive(Clone)]
#[repr(transparent)]
pub struct Runtime {
    pub(crate) inner: Ref<Mut<Inner>>,
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
        Self::new_with_alloc(crate::RustAllocator)
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
            inner: Ref::new(Mut::new(Inner {
                rt,
                info: None,
                #[cfg(feature = "allocator")]
                allocator,
                #[cfg(feature = "loader")]
                loader: None,
            })),
        };

        let opaque = Box::into_raw(Box::new(Opaque::new(&runtime)));
        unsafe { qjs::JS_SetRuntimeOpaque(rt, opaque as *mut _) };

        Ok(runtime)
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> WeakRuntime {
        WeakRuntime(Ref::downgrade(&self.inner))
    }

    pub fn set_promise_rejection_tracker(&self, handler: UncaughtPromiseRejectionHandler){
        
        unsafe extern "C" fn on_rejection(
            ctx_ptr: *mut JSContext,
            promise: JSValue,
            reason: JSValue,
            is_handled: ::std::os::raw::c_int,
            opaque: *mut ::std::os::raw::c_void,
        ) {

            let is_handled = is_handled == 1;
            let ctx = Ctx::from_ptr(ctx_ptr);

            let opaque = &mut *(opaque as *mut Opaque);
           
            let rejection = UncaughtPromiseReject{
                promise: Value::from_js_value_const(ctx,promise),
                reason: Value::from_js_value_const(ctx, reason)
            };

            if is_handled {
                opaque.uncaught_promise_rejections.remove(&rejection);
            }else{
                opaque.uncaught_promise_rejections.insert(rejection);
            }        
        }

        let mut guard = self.inner.lock();
        unsafe {
            qjs::JS_SetHostPromiseRejectionTracker(
                guard.rt,
                Some(on_rejection),
                qjs::JS_GetRuntimeOpaque(guard.rt),
            );
            guard.get_opaque_mut().uncaught_promise_rejection_handler = handler;
        }
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    /// If the provided closure returns `true` the interpreter will raise and uncatchable
    /// exception and return control flow to the caller.
    pub fn set_interrupt_handler(&self, handler: Option<Box<dyn FnMut() -> bool + 'static>>) {
        unsafe extern "C" fn interrupt_handler_trampoline(
            _rt: *mut qjs::JSRuntime,
            opaque: *mut ::std::os::raw::c_void,
        ) -> ::std::os::raw::c_int {
            let should_interrupt = match panic::catch_unwind(move || {
                let opaque = &mut *(opaque as *mut Opaque);
                opaque.interrupt_handler.as_mut().expect("handler is set")()
            }) {
                Ok(should_interrupt) => should_interrupt,
                Err(panic) => {
                    let opaque = &mut *(opaque as *mut Opaque);
                    opaque.panic = Some(panic);
                    // Returning true here will cause the interpreter to raise an un-catchable exception.
                    // The rust code that is running the interpreter will see that exception and continue
                    // the panic handling. See crate::result::{handle_exception, handle_panic} for details.
                    true
                }
            };
            should_interrupt as _
        }

        let mut guard = self.inner.lock();
        unsafe {
            qjs::JS_SetInterruptHandler(
                guard.rt,
                handler.as_ref().map(|_| interrupt_handler_trampoline as _),
                qjs::JS_GetRuntimeOpaque(guard.rt),
            );
            guard.get_opaque_mut().interrupt_handler = handler;
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
    ///
    /// Note that is a Noop when a custom allocator is being used,
    /// as is the case for the "rust-alloc" or "allocator" features.
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
    #[inline]
    pub fn is_job_pending(&self) -> bool {
        self.inner.lock().is_job_pending()
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    #[inline]
    pub fn execute_pending_job(&self) -> Result<bool> {
        self.inner.lock().execute_pending_job()
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
