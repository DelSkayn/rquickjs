use alloc::{
    ffi::CString,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{ptr::NonNull, result::Result as StdResult, task::Poll};

#[cfg(feature = "parallel")]
use std::sync::mpsc::{self, Sender};

use async_lock::Mutex;

use super::{
    opaque::Opaque, raw::RawRuntime, schedular::SchedularPoll, spawner::DriveFuture,
    InterruptHandler, JobErrorHandler, MemoryUsage, PromiseHook, RejectionTracker,
};
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{Loader, Resolver};
#[cfg(feature = "parallel")]
use crate::util::{AssertSendFuture, AssertSyncFuture};
use crate::{
    context::{AsyncContext, JobDrain},
    qjs,
    result::AsyncJobException,
    util::ManualPoll,
    Ctx, Result,
};

#[derive(Debug)]
pub(crate) struct InnerRuntime {
    pub runtime: RawRuntime,
}

#[cfg(feature = "parallel")]
unsafe impl Send for InnerRuntime {}

/// A weak handle to the async runtime.
///
/// Holding onto this struct does not prevent the runtime from being dropped.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[derive(Clone)]
pub struct AsyncWeakRuntime {
    inner: Weak<Mutex<InnerRuntime>>,
    #[cfg(feature = "parallel")]
    pending_free: Sender<NonNull<qjs::JSContext>>,
}

impl AsyncWeakRuntime {
    pub fn try_ref(&self) -> Option<AsyncRuntime> {
        self.inner.upgrade().map(|inner| AsyncRuntime {
            inner,
            #[cfg(feature = "parallel")]
            pending_free: self.pending_free.clone(),
        })
    }
}

/// Asynchronous QuickJS runtime, entry point of the library.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[derive(Clone)]
pub struct AsyncRuntime {
    // use Arc instead of Ref so we can use OwnedLock
    pub(crate) inner: Arc<Mutex<InnerRuntime>>,
    #[cfg(feature = "parallel")]
    pub(crate) pending_free: Sender<NonNull<qjs::JSContext>>,
}

// Since all functions which use runtime are behind a mutex
// sending the runtime to other threads should be fine.
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncRuntime {}
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncWeakRuntime {}

// Since a global lock needs to be locked for safe use
// using runtime in a sync way should be safe as
// simultaneous accesses is synchronized behind a lock.
#[cfg(feature = "parallel")]
unsafe impl Sync for AsyncRuntime {}
#[cfg(feature = "parallel")]
unsafe impl Sync for AsyncWeakRuntime {}

impl AsyncRuntime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    ///
    /// # Features
    /// *If the `"rust-alloc"` feature is enabled the Rust's global allocator will be used in favor of libc's one.*
    // Annoying false positive clippy lint
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Result<Self> {
        let opaque = Opaque::with_spawner();

        #[cfg(feature = "parallel")]
        let (pending_free, pending_free_recv) = mpsc::channel();
        let runtime = unsafe {
            RawRuntime::new(
                opaque,
                #[cfg(feature = "parallel")]
                pending_free_recv,
            )
        }?;

        Ok(Self {
            inner: Arc::new(Mutex::new(InnerRuntime { runtime })),
            #[cfg(feature = "parallel")]
            pending_free,
        })
    }

    /// Create a new runtime using specified allocator
    ///
    /// Will generally only fail if not enough memory was available.
    // Annoying false positive clippy lint
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new_with_alloc<A>(allocator: A) -> Result<Self>
    where
        A: Allocator + 'static,
    {
        let opaque = Opaque::with_spawner();

        #[cfg(feature = "parallel")]
        let (pending_free, pending_free_recv) = mpsc::channel();
        let runtime = unsafe {
            RawRuntime::new_with_allocator(
                opaque,
                allocator,
                #[cfg(feature = "parallel")]
                pending_free_recv,
            )
        }?;

        Ok(Self {
            inner: Arc::new(Mutex::new(InnerRuntime { runtime })),
            #[cfg(feature = "parallel")]
            pending_free,
        })
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> AsyncWeakRuntime {
        AsyncWeakRuntime {
            inner: Arc::downgrade(&self.inner),
            #[cfg(feature = "parallel")]
            pending_free: self.pending_free.clone(),
        }
    }

    /// Set a closure which is called when a Promise is rejected.
    #[inline]
    pub async fn set_host_promise_rejection_tracker(&self, tracker: Option<RejectionTracker>) {
        unsafe {
            self.inner
                .lock()
                .await
                .runtime
                .set_host_promise_rejection_tracker(tracker);
        }
    }

    /// Set a closure which is called when a job fails in an internal job
    /// drain ([`Ctx::execute_pending_job`], the `async_with` driver, the
    /// [`AsyncRuntime::drive`] future, or [`AsyncRuntime::idle`]).
    ///
    /// While a closure is set, failures are neither retained for
    /// [`Ctx::take_pending_job_error`] nor counted by
    /// [`Ctx::take_pending_job_error_overflow`]. Pass `None` to restore
    /// retention. Errors returned by [`AsyncRuntime::execute_pending_job`] do
    /// not reach this closure.
    ///
    /// The closure receives the failed job's context and an owned exception
    /// handle, or `undefined` if no exception value was thrown. If the job
    /// failed with a synthetic Rust panic exception, its stashed panic is
    /// resumed instead. Panics in the closure also propagate out of the drain.
    ///
    /// The closure runs synchronously mid-drain with the runtime lock held.
    /// It may re-enter the drain using the given context, but must not acquire
    /// the runtime lock again (e.g. by blocking on [`AsyncContext::async_with`]),
    /// which deadlocks. Catch or clear any exceptions raised by the closure
    /// before returning; otherwise they remain pending unless replaced by an
    /// uncatchable job error.
    ///
    /// Async drains leave uncatchable job errors pending after reporting or
    /// retaining them, so promise futures observe interrupted jobs even if
    /// their promises never settle. [`Ctx::execute_pending_job`] clears the
    /// failed job's exception instead.
    #[inline]
    pub async fn set_job_error_handler(&self, handler: Option<JobErrorHandler>) {
        self.inner
            .lock()
            .await
            .runtime
            .set_job_error_handler(handler);
    }

    /// Set a closure which is called when a promise is created, resolved, or chained.
    #[inline]
    pub async fn set_promise_hook(&self, tracker: Option<PromiseHook>) {
        unsafe {
            self.inner.lock().await.runtime.set_promise_hook(tracker);
        }
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    /// If the provided closure returns `true` the interpreter will raise and uncatchable
    /// exception and return control flow to the caller.
    #[inline]
    pub async fn set_interrupt_handler(&self, handler: Option<InterruptHandler>) {
        unsafe {
            self.inner
                .lock()
                .await
                .runtime
                .set_interrupt_handler(handler);
        }
    }

    /// Set the module loader
    #[cfg(feature = "loader")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
    pub async fn set_loader<R, L>(&self, resolver: R, loader: L)
    where
        R: Resolver + 'static,
        L: Loader + 'static,
    {
        unsafe {
            self.inner.lock().await.runtime.set_loader(resolver, loader);
        }
    }

    /// Set the info of the runtime
    pub async fn set_info<S: Into<Vec<u8>>>(&self, info: S) -> Result<()> {
        let string = CString::new(info)?;
        unsafe {
            self.inner.lock().await.runtime.set_info(string);
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
            self.inner.lock().await.runtime.set_memory_limit(limit);
        }
    }

    /// Set a limit on the max size of stack the runtime will use.
    ///
    /// The default values is 256x1024 bytes.
    pub async fn set_max_stack_size(&self, limit: usize) {
        unsafe {
            self.inner.lock().await.runtime.set_max_stack_size(limit);
        }
    }

    /// Set a memory threshold for garbage collection.
    pub async fn set_gc_threshold(&self, threshold: usize) {
        unsafe {
            self.inner.lock().await.runtime.set_gc_threshold(threshold);
        }
    }

    /// Manually run the garbage collection.
    ///
    /// Most QuickJS values are reference counted and
    /// will automatically free themselves when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub async fn run_gc(&self) {
        unsafe {
            let mut lock = self.inner.lock().await;
            lock.runtime.drain_pending_free();
            lock.runtime.run_gc();
        }
    }

    /// Get memory usage stats
    pub async fn memory_usage(&self) -> MemoryUsage {
        unsafe { self.inner.lock().await.runtime.memory_usage() }
    }

    /// Test for pending jobs
    ///
    /// Returns true when at least one job is pending.
    #[inline]
    pub async fn is_job_pending(&self) -> bool {
        let lock = self.inner.lock().await;

        lock.runtime.is_job_pending() || !lock.runtime.get_opaque().spawner_is_empty()
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    ///
    /// Failures are returned to the caller without invoking the job error
    /// handler, retaining the exception, or resuming stashed Rust panics.
    #[inline]
    pub async fn execute_pending_job(&self) -> StdResult<bool, AsyncJobException> {
        let mut lock = self.inner.lock().await;
        lock.runtime.update_stack_top();
        lock.runtime.drain_pending_free();

        let f = ManualPoll::new(|cx| {
            let job_res = lock.runtime.execute_pending_job().map_err(|e| {
                let ptr = NonNull::new(e)
                    .expect("executing pending job returned a null context on error");
                // JS_ExecutePendingJob returns a borrowed context pointer;
                // dup it so AsyncContext can own a reference.
                unsafe { qjs::JS_DupContext(ptr.as_ptr()) };
                AsyncJobException(unsafe { AsyncContext::from_raw(ptr, self.clone()) })
            })?;

            if job_res {
                return Poll::Ready(Ok(true));
            }

            match lock.runtime.get_opaque().poll(cx) {
                SchedularPoll::ShouldYield => Poll::Pending,
                SchedularPoll::Empty => Poll::Ready(Ok(false)),
                SchedularPoll::Pending => Poll::Ready(Ok(false)),
                SchedularPoll::PendingProgress => Poll::Ready(Ok(true)),
            }
        });

        #[cfg(feature = "parallel")]
        let f = unsafe { AssertSendFuture::assert(AssertSyncFuture::assert(f)) };

        f.await
    }

    /// Run all futures and jobs in the runtime until all are finished.
    ///
    /// Job failures are handled by [`AsyncRuntime::set_job_error_handler`] or
    /// retained for [`Ctx::take_pending_job_error`], and draining continues.
    /// Uncatchable errors remain pending for promise futures. Rust panics
    /// from failing callbacks or the handler propagate out of this future,
    /// leaving remaining work for another drain.
    #[inline]
    pub async fn idle(&self) {
        let mut lock = self.inner.lock().await;
        lock.runtime.update_stack_top();
        lock.runtime.drain_pending_free();

        let f = ManualPoll::new(|cx| {
            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(ctx) => {
                        let ctx = NonNull::new(ctx)
                            .expect("executing pending job returned a null context on error");
                        // Safety: the runtime is locked for this poll.
                        let job_ctx = unsafe { Ctx::from_raw(ctx) };
                        job_ctx.handle_failed_job(JobDrain::Async);
                        continue;
                    }
                }

                match lock.runtime.get_opaque().poll(cx) {
                    SchedularPoll::ShouldYield => return Poll::Pending,
                    SchedularPoll::Empty => return Poll::Ready(()),
                    SchedularPoll::Pending => return Poll::Pending,
                    SchedularPoll::PendingProgress => {}
                }
            }
        });

        #[cfg(feature = "parallel")]
        let f = unsafe { AssertSendFuture::assert(AssertSyncFuture::assert(f)) };

        f.await
    }

    /// Returns a future that completes when the runtime is dropped.
    /// If the future is polled it will drive futures spawned inside the runtime completing them
    /// even if runtime is currently not in use.
    ///
    /// Rust panics from failing jobs or the job error handler propagate out
    /// of the future's poll, terminating the driving task. Remaining work
    /// requires another drain, such as [`AsyncContext::async_with`].
    pub fn drive(&self) -> DriveFuture {
        DriveFuture::new(self.weak())
    }
}

#[cfg(test)]
macro_rules! async_test_case {
    ($name:ident => ($rt:ident,$ctx:ident) { $($t:tt)* }) => {
    #[test]
    fn $name() {
        #[cfg(feature = "parallel")]
        let mut new_thread = tokio::runtime::Builder::new_multi_thread();

        #[cfg(not(feature = "parallel"))]
        let mut new_thread = tokio::runtime::Builder::new_current_thread();

        let rt = new_thread
            .enable_all()
            .build()
            .unwrap();

        #[cfg(feature = "parallel")]
        {
            rt.block_on(async {
                let $rt = crate::AsyncRuntime::new().unwrap();
                let $ctx = crate::AsyncContext::full(&$rt).await.unwrap();

                $($t)*

            })
        }
        #[cfg(not(feature = "parallel"))]
        {
            let set = tokio::task::LocalSet::new();
            set.block_on(&rt, async {
                let $rt = crate::AsyncRuntime::new().unwrap();
                let $ctx = crate::AsyncContext::full(&$rt).await.unwrap();

                $($t)*
            })
        }
    }
    };
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::*;

    use self::context::EvalOptions;

    async_test_case!(basic => (_rt,ctx){
        ctx.async_with(async |ctx|{
            let res: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(res,2i32);
        }).await;
    });

    async_test_case!(sleep_closure => (_rt,ctx){

        let mut a = 1;
        let a_ref = &mut a;


        ctx.async_with(async |ctx|{
            tokio::time::sleep(Duration::from_secs_f64(0.01)).await;
            ctx.globals().set("foo","bar").unwrap();
            *a_ref += 1;
        }).await;
        assert_eq!(a,2);
    });

    async_test_case!(drive => (rt,ctx){
        use std::sync::{Arc, atomic::{Ordering,AtomicUsize}};

        #[cfg(feature = "parallel")]
        tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        tokio::task::spawn_local(rt.drive());

        // Give drive time to start.
        tokio::time::sleep(Duration::from_secs_f64(0.01)).await;

        let number = Arc::new(AtomicUsize::new(0));
        let number_clone = number.clone();
        let gate = Arc::new(tokio::sync::Notify::new());
        let gate_clone = gate.clone();
        let done = Arc::new(tokio::sync::Notify::new());
        let done_clone = done.clone();

        ctx.async_with(async |ctx|{
            ctx.spawn(async move {
                gate_clone.notified().await;
                number_clone.store(1,Ordering::SeqCst);
                done_clone.notify_one();
            });
        }).await;
        // Task is blocked on gate, so value is definitely still 0.
        assert_eq!(number.load(Ordering::SeqCst),0);
        // Unblock the task and wait for it to complete.
        gate.notify_one();
        done.notified().await;
        assert_eq!(number.load(Ordering::SeqCst),1);

    });

    async_test_case!(drive_retains_failed_job_error => (rt,ctx){
        #[cfg(feature = "parallel")]
        tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            // A raw job's failure would otherwise be unobservable.
            ctx.eval::<(),_>("queueMicrotask(() => { throw 5; });").unwrap();
            // Wake the drive task so its drain observes the queued job.
            ctx.spawn(async move {});
        }).await;

        // Wait for the drive future to drain the job, bounded so a
        // regression fails instead of hanging.
        let mut found = false;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            ctx.async_with(async |ctx|{
                if let Some(error) = ctx.take_pending_job_error() {
                    assert_eq!(error.as_int(), Some(5));
                    found = true;
                }
            }).await;
            if found {
                break;
            }
        }
        assert!(found, "the drive future did not run the failing job");

    });

    async_test_case!(drive_calls_job_error_handler => (rt,ctx){
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = seen.clone();
            rt.set_job_error_handler(Some(Box::new(move |_, error| {
                seen.lock().unwrap().push(error.as_int());
            }))).await;
        }

        #[cfg(feature = "parallel")]
        tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            ctx.eval::<(),_>("queueMicrotask(() => { throw 5; });").unwrap();
            // Wake the drive task so its drain observes the queued job.
            ctx.spawn(async move {});
        }).await;

        // Wait for the drive future to drain the job, bounded so a
        // regression fails instead of hanging.
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            if !seen.lock().unwrap().is_empty() {
                break;
            }
        }
        assert_eq!(*seen.lock().unwrap(), vec![Some(5)]);

        ctx.async_with(async |ctx|{
            assert!(ctx.take_pending_job_error().is_none());
            assert_eq!(ctx.take_pending_job_error_overflow(), 0);
            assert!(!ctx.has_exception());
        }).await;

    });

    async_test_case!(drive_leaves_interrupted_job_error_pending => (rt,ctx){
        use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

        let armed = Arc::new(AtomicBool::new(false));
        {
            let armed = armed.clone();
            rt.set_interrupt_handler(Some(Box::new(move || armed.load(Ordering::SeqCst)))).await;
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = seen.clone();
            rt.set_job_error_handler(Some(Box::new(move |_, error| {
                seen.lock().unwrap().push(error.is_uncatchable_error());
            }))).await;
        }

        #[cfg(feature = "parallel")]
        tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            ctx.eval::<(),_>("globalThis.p = Promise.resolve().then(() => { for (;;) {} });")
                .unwrap();
            armed.store(true, Ordering::SeqCst);
            // Wake the drive task so its drain observes the queued job.
            ctx.spawn(async move {});
        }).await;

        // Observe the drive task without draining jobs from this task.
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            if !seen.lock().unwrap().is_empty() {
                break;
            }
        }
        assert_eq!(*seen.lock().unwrap(), vec![true]);

        // The uncatchable error was left pending, so a future awaiting the
        // promise the job never settled bails out instead of hanging.
        tokio::time::timeout(Duration::from_secs(5), ctx.async_with(async |ctx|{
            let promise: Promise = ctx.globals().get("p").unwrap();
            let res = promise.into_future::<()>().await;
            assert!(matches!(res, Err(Error::Exception)));
            assert!(ctx.catch().is_uncatchable_error());
        }))
        .await
        .expect("the promise future hung on an interrupted job");

    });

    async_test_case!(drive_skips_caught_panic => (rt,ctx){
        #[cfg(feature = "parallel")]
        let handle = tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        let handle = tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            ctx.globals().set(
                "rustPanic",
                Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
            ).unwrap();
            // Catching the panic sentinel leaves its payload stashed, but the
            // unrelated job failure below must not resume it.
            ctx.eval::<(),_>("queueMicrotask(() => { try { rustPanic(); } catch (e) {} });").unwrap();
            ctx.eval::<(),_>("queueMicrotask(() => { throw 1; });").unwrap();
            // Wake the drive task so its drain observes the queued jobs.
            ctx.spawn(async move {});
        }).await;

        // Wait for the drive future to drain the jobs, bounded so a
        // regression fails instead of hanging.
        let mut found = false;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            ctx.async_with(async |ctx|{
                if let Some(error) = ctx.take_pending_job_error() {
                    assert_eq!(error.as_int(), Some(1));
                    found = true;
                }
            }).await;
            if found {
                break;
            }
        }
        assert!(found, "the drive future did not run the failing job");

        assert!(!handle.is_finished());

    });

    async_test_case!(drive_resumes_pending_panic => (rt,ctx){
        #[cfg(feature = "parallel")]
        let handle = tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        let handle = tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            ctx.globals().set(
                "rustPanic",
                Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
            ).unwrap();
            ctx.eval::<(),_>("queueMicrotask(rustPanic);").unwrap();
            // Wake the drive task so its drain observes the queued job even
            // if it already polled once.
            ctx.spawn(async move {});
        }).await;

        // The drive task's drain resumes the stashed panic; tokio catches
        // the unwind at the task boundary and the task dies with the
        // payload.
        let err = handle.await.expect_err("the drive task should have panicked");
        assert_eq!(*err.into_panic().downcast_ref::<&str>().unwrap(), "rust boom");

    });

    async_test_case!(drive_keeps_draining_after_failure => (rt,ctx){
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = seen.clone();
            rt.set_job_error_handler(Some(Box::new(move |_, error| {
                seen.lock().unwrap().push(error.as_int());
            }))).await;
        }

        #[cfg(feature = "parallel")]
        tokio::spawn(rt.drive());
        #[cfg(not(feature = "parallel"))]
        tokio::task::spawn_local(rt.drive());

        ctx.async_with(async |ctx|{
            ctx.eval::<(),_>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });
                 queueMicrotask(() => { throw 3; });
                 queueMicrotask(() => { globalThis.ranLast = true; });",
            ).unwrap();
            // Wake the drive task so its drain observes the queued jobs.
            ctx.spawn(async move {});
        }).await;

        // Observe the drive task without draining jobs from this task.
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            if seen.lock().unwrap().len() == 3 && !rt.is_job_pending().await {
                break;
            }
        }
        assert_eq!(*seen.lock().unwrap(), vec![Some(1), Some(2), Some(3)]);
        assert!(!rt.is_job_pending().await);
        ctx.with(|ctx| {
            let ran_last: bool = ctx.globals().get("ranLast").unwrap();
            assert!(ran_last);
        }).await;

    });

    async_test_case!(job_failing_in_spawned_future_drop_does_not_outlive_runtime => (rt,ctx){
        struct DrainOnDrop<'js>(Ctx<'js>);

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

        ctx.with(|ctx| {
            let guard = DrainOnDrop(ctx.clone());
            ctx.spawn(async move {
                let _guard = guard;
                std::future::pending::<()>().await
            });
        }).await;
        drop(ctx);
        drop(rt);
    });

    async_test_case!(no_drive => (rt,ctx){
        use std::sync::{Arc, atomic::{Ordering,AtomicUsize}};

        let number = Arc::new(AtomicUsize::new(0));
        let number_clone = number.clone();

        ctx.async_with(async |ctx|{
            ctx.spawn(async move {
                tokio::task::yield_now().await;
                number_clone.store(1,Ordering::SeqCst);
            });
        }).await;
        assert_eq!(number.load(Ordering::SeqCst),0);
        tokio::time::sleep(Duration::from_secs_f64(0.01)).await;
        assert_eq!(number.load(Ordering::SeqCst),0);

    });

    async_test_case!(idle => (rt,ctx){
        use std::sync::{Arc, atomic::{Ordering,AtomicUsize}};

        let number = Arc::new(AtomicUsize::new(0));
        let number_clone = number.clone();

        ctx.async_with(async |ctx|{
            ctx.spawn(async move {
                tokio::task::yield_now().await;
                number_clone.store(1,Ordering::SeqCst);
            });
        }).await;
        assert_eq!(number.load(Ordering::SeqCst),0);
        rt.idle().await;
        assert_eq!(number.load(Ordering::SeqCst),1);

    });

    async_test_case!(idle_calls_job_error_handler => (rt,ctx){
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = seen.clone();
            rt.set_job_error_handler(Some(Box::new(move |_, error| {
                seen.lock().unwrap().push(error.as_int());
            }))).await;
        }

        ctx.with(|ctx| {
            ctx.eval::<(),_>("queueMicrotask(() => { throw 5; });").unwrap();
        }).await;

        tokio::time::timeout(Duration::from_secs(5), rt.idle())
            .await
            .expect("idle hung");

        assert_eq!(*seen.lock().unwrap(), vec![Some(5)]);
        ctx.with(|ctx| {
            assert!(ctx.take_pending_job_error().is_none());
            assert_eq!(ctx.take_pending_job_error_overflow(), 0);
            assert!(!ctx.has_exception());
        }).await;
    });

    async_test_case!(idle_keeps_draining_after_failure => (rt,ctx){
        ctx.with(|ctx| {
            ctx.eval::<(),_>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });
                 queueMicrotask(() => { globalThis.ranLast = true; });",
            ).unwrap();
        }).await;

        tokio::time::timeout(Duration::from_secs(5), rt.idle())
            .await
            .expect("idle hung");

        assert!(!rt.is_job_pending().await);
        ctx.with(|ctx| {
            let ran_last: bool = ctx.globals().get("ranLast").unwrap();
            assert!(ran_last);
            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(1));
            assert_eq!(ctx.take_pending_job_error_overflow(), 1);
            assert!(!ctx.has_exception());
        }).await;
    });

    async_test_case!(idle_resumes_pending_panic => (rt,ctx){
        use futures_rs::FutureExt;
        use std::panic::AssertUnwindSafe;

        ctx.with(|ctx| {
            ctx.globals().set(
                "rustPanic",
                Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
            ).unwrap();
            ctx.eval::<(),_>("queueMicrotask(rustPanic);").unwrap();
        }).await;

        let res = tokio::time::timeout(
            Duration::from_secs(5),
            AssertUnwindSafe(rt.idle()).catch_unwind(),
        )
        .await
        .expect("idle hung");
        let payload = res.expect_err("idle should resume the stashed panic");
        assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "rust boom");

        ctx.with(|ctx| {
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        }).await;
    });

    async_test_case!(idle_propagates_job_error_handler_panic => (rt,ctx){
        use futures_rs::FutureExt;
        use std::panic::AssertUnwindSafe;

        rt.set_job_error_handler(Some(Box::new(|_, _| panic!("handler boom")))).await;

        ctx.with(|ctx| {
            ctx.eval::<(),_>("queueMicrotask(() => { throw 1; });").unwrap();
        }).await;

        let res = tokio::time::timeout(
            Duration::from_secs(5),
            AssertUnwindSafe(rt.idle()).catch_unwind(),
        )
        .await
        .expect("idle hung");
        let payload = res.expect_err("the handler panic should propagate out of idle");
        assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "handler boom");

        ctx.with(|ctx| {
            assert!(ctx.take_pending_job_error().is_none());
            assert!(!ctx.has_exception());
        }).await;
    });

    async_test_case!(recursive_spawn => (rt,ctx){
        use tokio::sync::oneshot;

        ctx.async_with(async |ctx|{
            let ctx_clone = ctx.clone();
            let (tx,rx) = oneshot::channel::<()>();
            let (tx2,rx2) = oneshot::channel::<()>();
            ctx.spawn(async move {
                tokio::task::yield_now().await;

                let ctx = ctx_clone.clone();

                ctx_clone.spawn(async move {
                    tokio::task::yield_now().await;
                    ctx.spawn(async move {
                        tokio::task::yield_now().await;
                        tx2.send(()).unwrap();
                        tokio::task::yield_now().await;
                    });
                    tokio::task::yield_now().await;
                    tx.send(()).unwrap();
                });

                // Add a bunch of futures just to make sure possible segfaults are more likely to
                // happen
                for _ in 0..32{
                    ctx_clone.spawn(async move {})
                }

            });
            tokio::time::timeout(Duration::from_millis(500), rx).await.unwrap().unwrap();
            tokio::time::timeout(Duration::from_millis(500), rx2).await.unwrap().unwrap();
        }).await;

    });

    async_test_case!(recursive_spawn_from_script => (rt,ctx) {
        use std::sync::atomic::{Ordering, AtomicUsize};
        use crate::prelude::Func;

        static COUNT: AtomicUsize = AtomicUsize::new(0);
        static SCRIPT: &str = r#"

        async function main() {

          setTimeout(() => {
            inc_count()
            setTimeout(async () => {
                inc_count()
            }, 100);
          }, 100);
        }

        main().catch(print);


        "#;

        fn inc_count(){
            COUNT.fetch_add(1,Ordering::Relaxed);
        }

        fn set_timeout_spawn<'js>(ctx: Ctx<'js>, callback: Function<'js>, millis: usize) -> Result<()> {
            ctx.spawn(async move {
                tokio::time::sleep(Duration::from_millis(millis as u64)).await;
                callback.call::<_, ()>(()).unwrap();
            });

            Ok(())
        }


        ctx.async_with(async |ctx|{

            let res: Result<Promise> = (|| {
                let globals = ctx.globals();

                globals.set("inc_count", Func::from(inc_count))?;

                globals.set("setTimeout", Func::from(set_timeout_spawn))?;
                let options = EvalOptions{
                    promise: true,
                    strict: false,
                    ..EvalOptions::default()
                };

                ctx.eval_with_options(SCRIPT, options)?
            })();

            match res.catch(&ctx){
                Ok(promise) => {
                    if let Err(err) = promise.into_future::<Value>().await.catch(&ctx){
                        eprintln!("{}", err)
                    }
                },
                Err(err) => {
                    eprintln!("{}", err)
                },
            };

        })
        .await;

        rt.idle().await;

        assert_eq!(COUNT.load(Ordering::Relaxed),2);
    });

    async_test_case!(interrupt_handler_idle => (rt, ctx) {
        use std::time::Instant;

        let timeout = Duration::from_millis(100);
        let start_time = Instant::now();

        rt.set_interrupt_handler(Some(Box::new(move || start_time.elapsed() >= timeout)))
            .await;

        let _ = ctx.async_with(async |ctx| {
            ctx.eval::<(), _>(r#"
                async function example() {
                    while (true) {
                        await Promise.resolve();
                    }
                }
                example();
            "#)
        }).await;

        // This previously caused an assertion failure in gc_decref_child
        // due to the interrupt handler corrupting reference counts during
        // pending job execution.
        rt.idle().await;
    });

    #[cfg(feature = "parallel")]
    fn assert_is_send<T: Send>(t: T) -> T {
        t
    }

    #[cfg(feature = "parallel")]
    fn assert_is_sync<T: Send>(t: T) -> T {
        t
    }

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn ensure_types_are_send_sync() {
        let rt = AsyncRuntime::new().unwrap();

        std::mem::drop(assert_is_sync(rt.idle()));
        std::mem::drop(assert_is_sync(rt.execute_pending_job()));
        std::mem::drop(assert_is_sync(rt.drive()));

        std::mem::drop(assert_is_send(rt.idle()));
        std::mem::drop(assert_is_send(rt.execute_pending_job()));
        std::mem::drop(assert_is_send(rt.drive()));
    }
}
