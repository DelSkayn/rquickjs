use alloc::{
    ffi::CString,
    sync::{Arc, Weak},
    vec::Vec,
};
#[cfg(feature = "parallel")]
use core::ptr::NonNull;
use core::{result::Result as StdResult, task::Poll};
#[cfg(feature = "parallel")]
use std::sync::mpsc::{self, Sender};

use async_lock::Mutex;

use super::{
    job_error::JobError, opaque::Opaque, raw::RawRuntime, schedular::SchedularPoll,
    spawner::DriveFuture, InterruptHandler, MemoryUsage, PromiseHook, RejectionTracker,
};
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{Loader, Resolver};
#[cfg(feature = "parallel")]
use crate::qjs;
#[cfg(feature = "parallel")]
use crate::util::{AssertSendFuture, AssertSyncFuture};
use crate::{util::ManualPoll, Result};

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

    /// Run all futures and jobs in the runtime until all are finished.
    ///
    /// Returns the first failed job's exception as a [`JobError`]. Calling this
    /// again continues with the jobs left in the queue. A Rust panic raised
    /// inside a job is resumed at the failure site.
    #[inline]
    pub async fn idle(&self) -> StdResult<(), JobError> {
        let lock = self.inner.lock().await;
        lock.runtime.update_stack_top();
        lock.runtime.drain_pending_free();
        unsafe { lock.runtime.get_opaque().drain_job_errors() };

        let f = ManualPoll::new(|cx| loop {
            match lock.runtime.execute_pending_job() {
                Ok(false) => {}
                Ok(true) => continue,
                Err(err) => return Poll::Ready(Err(err)),
            }

            match lock.runtime.get_opaque().poll(cx) {
                SchedularPoll::ShouldYield => return Poll::Pending,
                SchedularPoll::Empty => return Poll::Ready(Ok(())),
                SchedularPoll::Pending => return Poll::Pending,
                SchedularPoll::PendingProgress => {}
            }
        });

        #[cfg(feature = "parallel")]
        let f = unsafe { AssertSendFuture::assert(AssertSyncFuture::assert(f)) };

        f.await
    }

    /// Returns a future that drives futures spawned inside the runtime even when
    /// the runtime is not otherwise in use.
    ///
    /// The future resolves with `Ok(())` when the runtime is dropped, or with the
    /// first failed job's exception as [`JobError`]. Poll it again to continue
    /// driving the remaining queue.
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
        }).await.unwrap();
    });

    async_test_case!(sleep_closure => (_rt,ctx){

        let mut a = 1;
        let a_ref = &mut a;


        ctx.async_with(async |ctx|{
            tokio::time::sleep(Duration::from_secs_f64(0.01)).await;
            ctx.globals().set("foo","bar").unwrap();
            *a_ref += 1;
        }).await.unwrap();
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
        }).await.unwrap();
        // Task is blocked on gate, so value is definitely still 0.
        assert_eq!(number.load(Ordering::SeqCst),0);
        // Unblock the task and wait for it to complete.
        gate.notify_one();
        done.notified().await;
        assert_eq!(number.load(Ordering::SeqCst),1);

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
        }).await.unwrap();
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
        }).await.unwrap();
        assert_eq!(number.load(Ordering::SeqCst),0);
        rt.idle().await.unwrap();
        assert_eq!(number.load(Ordering::SeqCst),1);

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
        }).await.unwrap();

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
        .await.unwrap();

        rt.idle().await.unwrap();

        assert_eq!(COUNT.load(Ordering::Relaxed),2);
    });

    async_test_case!(idle_returns_failed_job => (rt, ctx) {
        ctx.async_with(async |ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('idle failure'); })").unwrap();
        }).await.unwrap();

        let err = rt.idle().await.unwrap_err();
        ctx.with(|ctx| {
            ctx.globals().set("err", err.into_value(&ctx).unwrap()).unwrap();
            assert_eq!(ctx.eval::<std::string::String, _>("err.message").unwrap(), "idle failure");
        }).await;
    });

    async_test_case!(async_with_returns_failed_job => (rt, ctx) {
        let err = ctx.async_with(async |ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('with failure'); })").unwrap();
            tokio::task::yield_now().await;
        }).await.unwrap_err();

        ctx.with(|ctx| {
            ctx.globals().set("err", err.into_value(&ctx).unwrap()).unwrap();
            assert_eq!(ctx.eval::<std::string::String, _>("err.message").unwrap(), "with failure");
        }).await;
    });

    async_test_case!(drive_returns_failed_job => (rt, ctx) {
        ctx.async_with(async |ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('drive failure'); })").unwrap();
        }).await.unwrap();

        let err = rt.drive().await.unwrap_err();
        ctx.with(|ctx| {
            ctx.globals().set("err", err.into_value(&ctx).unwrap()).unwrap();
            assert_eq!(ctx.eval::<std::string::String, _>("err.message").unwrap(), "drive failure");
        }).await;
    });

    async_test_case!(reentering_idle_continues_the_queue => (rt, ctx) {
        ctx.async_with(async |ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw new Error('one'); });\
                 queueMicrotask(() => { globalThis.done = 1; });",
            ).unwrap();
        }).await.unwrap();

        let first = rt.idle().await.unwrap_err();
        // The next drain releases unread exceptions, so consume this one first.
        ctx.with(|ctx| {
            ctx.globals().set("err", first.into_value(&ctx).unwrap()).unwrap();
        }).await;
        rt.idle().await.unwrap();
        ctx.with(|ctx| {
            assert_eq!(ctx.eval::<std::string::String, _>("err.message").unwrap(), "one");
            assert_eq!(ctx.eval::<i32, _>("globalThis.done").unwrap(), 1);
        }).await;
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
        // pending job execution. The interrupt may fail a job, so the result
        // is intentionally ignored.
        let _ = rt.idle().await;
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
        std::mem::drop(assert_is_sync(rt.drive()));

        std::mem::drop(assert_is_send(rt.idle()));
        std::mem::drop(assert_is_send(rt.drive()));

        fn assert_job_error_is_send<T: Send>() {}
        assert_job_error_is_send::<JobError>();
    }
}
