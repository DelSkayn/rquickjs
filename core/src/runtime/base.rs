//! QuickJS runtime related types.

use super::{
    opaque::Opaque, raw::RawRuntime, InterruptHandler, JobErrorHandler, MemoryUsage, PromiseHook,
    RejectionTracker,
};
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{Loader, Resolver};
use crate::{qjs, result::JobException, Context, Ctx, Mut, Ref, Result, Weak};
use alloc::{ffi::CString, vec::Vec};
use core::{ptr::NonNull, result::Result as StdResult};
#[cfg(feature = "parallel")]
use std::sync::mpsc::{self, Sender};

/// A weak handle to the runtime.
///
/// Holding onto this struct does not prevent the runtime from being dropped.
#[derive(Clone)]
pub struct WeakRuntime {
    inner: Weak<Mut<RawRuntime>>,
    #[cfg(feature = "parallel")]
    pending_free: Sender<NonNull<qjs::JSContext>>,
}

impl WeakRuntime {
    pub fn try_ref(&self) -> Option<Runtime> {
        self.inner.upgrade().map(|inner| Runtime {
            inner,
            #[cfg(feature = "parallel")]
            pending_free: self.pending_free.clone(),
        })
    }
}

/// QuickJS runtime, entry point of the library.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) inner: Ref<Mut<RawRuntime>>,
    #[cfg(feature = "parallel")]
    pub(crate) pending_free: Sender<NonNull<qjs::JSContext>>,
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
        #[cfg(feature = "parallel")]
        let (pending_free, pending_free_recv) = mpsc::channel();
        let rt = unsafe {
            RawRuntime::new(
                opaque,
                #[cfg(feature = "parallel")]
                pending_free_recv,
            )?
        };
        Ok(Self {
            inner: Ref::new(Mut::new(rt)),
            #[cfg(feature = "parallel")]
            pending_free,
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
        #[cfg(feature = "parallel")]
        let (pending_free, pending_free_recv) = mpsc::channel();
        let rt = unsafe {
            RawRuntime::new_with_allocator(
                opaque,
                allocator,
                #[cfg(feature = "parallel")]
                pending_free_recv,
            )?
        };
        Ok(Self {
            inner: Ref::new(Mut::new(rt)),
            #[cfg(feature = "parallel")]
            pending_free,
        })
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> WeakRuntime {
        WeakRuntime {
            inner: Ref::downgrade(&self.inner),
            #[cfg(feature = "parallel")]
            pending_free: self.pending_free.clone(),
        }
    }

    /// Set a closure which is called when a promise is created, resolved, or chained.
    #[inline]
    pub fn set_promise_hook(&self, tracker: Option<PromiseHook>) {
        unsafe {
            self.inner.lock().set_promise_hook(tracker);
        }
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

    /// Runs mid-drain with the runtime lock held; do not acquire it again.
    #[inline]
    pub fn set_job_error_handler(&self, handler: Option<JobErrorHandler>) {
        self.inner.lock().set_job_error_handler(handler);
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
    ///
    /// A failure is returned to the caller; a stashed Rust panic is resumed.
    #[inline]
    pub fn execute_pending_job(&self) -> StdResult<bool, JobException> {
        let mut lock = self.inner.lock();
        lock.update_stack_top();
        let mut panic = None;
        let res = lock.execute_pending_job().map_err(|e| {
            let ptr = NonNull::new(e).expect("QuickJS returned null ptr for job error");
            panic = unsafe { Ctx::from_raw(ptr) }.take_job_panic();
            // JS_ExecutePendingJob returns a borrowed context pointer;
            // dup it so Context can own a reference.
            unsafe { qjs::JS_DupContext(ptr.as_ptr()) };
            JobException(unsafe { Context::from_raw(ptr, self.clone()) })
        });
        // Unwinding under the lock would poison the runtime.
        drop(lock);
        if let Some(payload) = panic {
            crate::util::resume_unwind(payload);
        }
        res
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

    #[test]
    fn job_failing_in_userdata_drop_does_not_outlive_runtime() {
        use crate::{Context, Ctx, JsLifetime};

        struct DrainOnDrop<'js>(Ctx<'js>);

        unsafe impl<'js> JsLifetime<'js> for DrainOnDrop<'js> {
            type Changed<'to> = DrainOnDrop<'to>;
        }

        impl Drop for DrainOnDrop<'_> {
            fn drop(&mut self) {
                self.0
                    .eval::<(), _>("queueMicrotask(() => { throw { late: true }; })")
                    .unwrap();
                while self.0.execute_pending_job() {}
                assert!(self.0.take_pending_job_error().is_none());
                assert_eq!(self.0.take_pending_job_error_overflow(), 0);
            }
        }

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.store_userdata(DrainOnDrop(ctx.clone())).unwrap();
        });
        drop(ctx);
        drop(rt);
    }

    #[test]
    fn retained_job_error_is_released_before_userdata() {
        use crate::{Context, Ctx, Function, JsLifetime};
        use std::{cell::Cell, rc::Rc};

        struct Marker;

        unsafe impl<'js> JsLifetime<'js> for Marker {
            type Changed<'to> = Marker;
        }

        struct CheckOnDrop<'js>(Ctx<'js>, Rc<Cell<Option<bool>>>);

        impl Drop for CheckOnDrop<'_> {
            fn drop(&mut self) {
                self.1.set(Some(self.0.userdata::<Marker>().is_some()));
            }
        }

        let saw_userdata = Rc::new(Cell::new(None));
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.store_userdata(Marker).unwrap();
            let guard = CheckOnDrop(ctx.clone(), saw_userdata.clone());
            let f = Function::new(ctx.clone(), move || {
                let _guard = &guard;
            })
            .unwrap();
            ctx.globals().set("f", f).unwrap();
            ctx.eval::<(), _>(
                "queueMicrotask(() => { const f = globalThis.f; delete globalThis.f; throw f; });",
            )
            .unwrap();
            while ctx.execute_pending_job() {}
        });
        assert_eq!(saw_userdata.get(), None);

        drop(ctx);
        drop(rt);
        assert_eq!(saw_userdata.get(), Some(true));
    }

    #[test]
    fn set_max_stack_size_large_values() {
        let rt = Runtime::new().unwrap();
        rt.set_max_stack_size(usize::MAX);
        let ctx = crate::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<i32, _>("1 + 1").unwrap();
        });
        rt.set_max_stack_size(isize::MAX as usize);
        ctx.with(|ctx| {
            ctx.eval::<i32, _>("1 + 1").unwrap();
        });
        rt.set_max_stack_size(0);
        ctx.with(|ctx| {
            ctx.eval::<i32, _>("1 + 1").unwrap();
        });
        rt.set_max_stack_size(256 * 1024);
        ctx.with(|ctx| {
            ctx.eval::<i32, _>("1 + 1").unwrap();
        });
    }

    #[test]
    fn runtime_execute_pending_job_resumes_pending_panic() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let rt = Runtime::new().unwrap();
        let ctx = crate::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set(
                    "rustPanic",
                    crate::Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
                )
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(rustPanic);").unwrap();
        });

        let payload = catch_unwind(AssertUnwindSafe(|| {
            let _ = rt.execute_pending_job();
        }))
        .expect_err("the drain should resume the stashed panic");
        assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "rust boom");

        ctx.with(|ctx| {
            assert!(matches!(
                ctx.eval::<(), _>("throw 1"),
                Err(crate::Error::Exception)
            ));
            assert_eq!(ctx.catch().as_int(), Some(1));
            let sum: i32 = ctx.eval::<i32, _>("1 + 1").unwrap();
            assert_eq!(sum, 2);
        });
    }

    #[test]
    fn job_panicking_during_teardown_does_not_unwind() {
        use crate::{Ctx, Function, JsLifetime};

        struct DrainOnDrop<'js>(Ctx<'js>);

        unsafe impl<'js> JsLifetime<'js> for DrainOnDrop<'js> {
            type Changed<'to> = DrainOnDrop<'to>;
        }

        impl Drop for DrainOnDrop<'_> {
            fn drop(&mut self) {
                self.0.eval::<(), _>("queueMicrotask(rustPanic);").unwrap();
                while self.0.execute_pending_job() {}
            }
        }

        let rt = Runtime::new().unwrap();
        let ctx = crate::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set(
                    "rustPanic",
                    Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
                )
                .unwrap();
            ctx.store_userdata(DrainOnDrop(ctx.clone())).unwrap();
        });
        drop(ctx);
        drop(rt);
    }

    #[test]
    fn prototypes_registered_during_teardown_are_dropped() {
        use crate::{Ctx, Function, JsLifetime};

        struct RegistersPrototype<'js>(Ctx<'js>);

        unsafe impl<'js> JsLifetime<'js> for RegistersPrototype<'js> {
            type Changed<'to> = RegistersPrototype<'to>;
        }

        impl Drop for RegistersPrototype<'_> {
            fn drop(&mut self) {
                Function::new(self.0.clone(), || 1).unwrap();
            }
        }

        let rt = Runtime::new().unwrap();
        let ctx = crate::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.store_userdata(RegistersPrototype(ctx.clone())).unwrap();
        });
        drop(ctx);
        drop(rt);
    }

    #[test]
    fn context_dropped_while_lock_held() {
        let rt = Runtime::new().unwrap();
        let ctx1 = crate::Context::full(&rt).unwrap();
        let ctx2 = crate::Context::full(&rt).unwrap();

        ctx1.with(|_| {
            drop(ctx2);
        });
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn context_parked_by_other_thread_is_still_freed() {
        use std::sync::{Arc, Barrier};
        use std::{thread, time::Duration};

        let rt = Runtime::new().unwrap();
        let ctx1 = crate::Context::full(&rt).unwrap();
        let ctx2 = crate::Context::full(&rt).unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let barrier_holder = barrier.clone();
        let holder = thread::spawn(move || {
            ctx1.with(|_| {
                barrier_holder.wait();
                thread::sleep(Duration::from_millis(100));
            });
        });

        barrier.wait();
        // The lock is held by the other thread, so this parks rather than
        // freeing. Tearing the runtime down afterwards has to release it;
        // `JS_FreeRuntime` aborts if any context is still alive.
        drop(ctx2);

        holder.join().unwrap();
        drop(rt);
    }
}
