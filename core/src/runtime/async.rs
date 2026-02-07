use alloc::{ffi::CString, vec::Vec};
use core::{ptr::NonNull, result::Result as StdResult, task::Poll};

#[cfg(feature = "parallel")]
use alloc::sync::Arc;
#[cfg(feature = "parallel")]
use async_lock::Mutex;
#[cfg(feature = "parallel")]
use std::sync::mpsc::{self, Receiver, Sender};

#[cfg(not(feature = "parallel"))]
use alloc::rc::Rc;
#[cfg(not(feature = "parallel"))]
use core::cell::RefCell;

use super::{
    opaque::Opaque, raw::RawRuntime, spawner::DriveFuture, task_queue::TaskPoll, InterruptHandler,
    MemoryUsage, PromiseHook, RejectionTracker,
};
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{Loader, Resolver};
#[cfg(feature = "parallel")]
use crate::qjs;
use crate::{context::AsyncContext, result::AsyncJobException, Ctx, Result};

// Type aliases for lock abstraction
#[cfg(feature = "parallel")]
pub(crate) type RuntimeLock<T> = Mutex<T>;
#[cfg(not(feature = "parallel"))]
pub(crate) type RuntimeLock<T> = RefCell<T>;

#[cfg(feature = "parallel")]
pub(crate) type RuntimeRef<T> = Arc<T>;
#[cfg(not(feature = "parallel"))]
pub(crate) type RuntimeRef<T> = Rc<T>;

#[cfg(feature = "parallel")]
pub(crate) type RuntimeWeak<T> = alloc::sync::Weak<T>;
#[cfg(not(feature = "parallel"))]
pub(crate) type RuntimeWeak<T> = alloc::rc::Weak<T>;

// Guard type aliases
#[cfg(feature = "parallel")]
pub(crate) type RuntimeGuard<'a, T> = async_lock::MutexGuard<'a, T>;
#[cfg(not(feature = "parallel"))]
pub(crate) type RuntimeGuard<'a, T> = core::cell::RefMut<'a, T>;

#[derive(Debug)]
pub(crate) struct InnerRuntime {
    pub runtime: RawRuntime,
    #[cfg(feature = "parallel")]
    pub drop_recv: Receiver<NonNull<qjs::JSContext>>,
}

impl InnerRuntime {
    #[inline]
    pub fn drop_pending(&self) {
        #[cfg(feature = "parallel")]
        while let Ok(x) = self.drop_recv.try_recv() {
            unsafe { qjs::JS_FreeContext(x.as_ptr()) }
        }
    }
}

impl Drop for InnerRuntime {
    fn drop(&mut self) {
        self.drop_pending();
    }
}

#[cfg(feature = "parallel")]
unsafe impl Send for InnerRuntime {}

/// A weak handle to the async runtime.
///
/// Holding onto this struct does not prevent the runtime from being dropped.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[derive(Clone)]
pub struct AsyncWeakRuntime {
    pub(crate) inner: RuntimeWeak<RuntimeLock<InnerRuntime>>,
    #[cfg(feature = "parallel")]
    pub(crate) drop_send: Sender<NonNull<qjs::JSContext>>,
}

impl AsyncWeakRuntime {
    pub fn try_ref(&self) -> Option<AsyncRuntime> {
        self.inner.upgrade().map(|inner| AsyncRuntime {
            inner,
            #[cfg(feature = "parallel")]
            drop_send: self.drop_send.clone(),
        })
    }
}

/// Asynchronous QuickJS runtime, entry point of the library.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[derive(Clone)]
pub struct AsyncRuntime {
    pub(crate) inner: RuntimeRef<RuntimeLock<InnerRuntime>>,
    #[cfg(feature = "parallel")]
    pub(crate) drop_send: Sender<NonNull<qjs::JSContext>>,
}

#[cfg(feature = "parallel")]
unsafe impl Send for AsyncRuntime {}
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncWeakRuntime {}
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
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Result<Self> {
        Self::new_inner(unsafe { RawRuntime::new(Opaque::with_spawner()) }?)
    }

    /// Create a new runtime using specified allocator.
    ///
    /// Will generally only fail if not enough memory was available.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new_with_alloc<A: Allocator + 'static>(allocator: A) -> Result<Self> {
        Self::new_inner(unsafe {
            RawRuntime::new_with_allocator(Opaque::with_spawner(), allocator)
        }?)
    }

    fn new_inner(runtime: RawRuntime) -> Result<Self> {
        #[cfg(feature = "parallel")]
        let (drop_send, drop_recv) = mpsc::channel();

        Ok(Self {
            inner: RuntimeRef::new(RuntimeLock::new(InnerRuntime {
                runtime,
                #[cfg(feature = "parallel")]
                drop_recv,
            })),
            #[cfg(feature = "parallel")]
            drop_send,
        })
    }

    /// Get weak ref to runtime.
    pub fn weak(&self) -> AsyncWeakRuntime {
        AsyncWeakRuntime {
            #[cfg(feature = "parallel")]
            inner: Arc::downgrade(&self.inner),
            #[cfg(not(feature = "parallel"))]
            inner: Rc::downgrade(&self.inner),
            #[cfg(feature = "parallel")]
            drop_send: self.drop_send.clone(),
        }
    }

    // Lock helpers - zero-cost for non-parallel
    #[cfg(feature = "parallel")]
    pub(crate) async fn lock(&self) -> RuntimeGuard<'_, InnerRuntime> {
        self.inner.lock().await
    }

    #[cfg(not(feature = "parallel"))]
    pub(crate) async fn lock(&self) -> RuntimeGuard<'_, InnerRuntime> {
        self.inner.borrow_mut()
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn try_lock(&self) -> Option<RuntimeGuard<'_, InnerRuntime>> {
        self.inner.try_lock()
    }

    #[cfg(not(feature = "parallel"))]
    pub(crate) fn try_lock(&self) -> Option<RuntimeGuard<'_, InnerRuntime>> {
        self.inner.try_borrow_mut().ok()
    }

    /// Set a closure which is called when a promise is rejected.
    pub async fn set_host_promise_rejection_tracker(&self, tracker: Option<RejectionTracker>) {
        unsafe {
            self.lock()
                .await
                .runtime
                .set_host_promise_rejection_tracker(tracker)
        }
    }

    /// Set a closure which is called when a promise is created, resolved, or chained.
    pub async fn set_promise_hook(&self, tracker: Option<PromiseHook>) {
        unsafe { self.lock().await.runtime.set_promise_hook(tracker) }
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    ///
    /// If the provided closure returns `true` the interpreter will raise an uncatchable
    /// exception and return control flow to the caller.
    pub async fn set_interrupt_handler(&self, handler: Option<InterruptHandler>) {
        unsafe { self.lock().await.runtime.set_interrupt_handler(handler) }
    }

    /// Set the module loader.
    #[cfg(feature = "loader")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
    pub async fn set_loader<R: Resolver + 'static, L: Loader + 'static>(
        &self,
        resolver: R,
        loader: L,
    ) {
        unsafe { self.lock().await.runtime.set_loader(resolver, loader) }
    }

    /// Set the info of the runtime.
    pub async fn set_info<S: Into<Vec<u8>>>(&self, info: S) -> Result<()> {
        unsafe { self.lock().await.runtime.set_info(CString::new(info)?) };
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    ///
    /// Note that is a Noop when a custom allocator is being used,
    /// as is the case for the `"rust-alloc"` or `"allocator"` features.
    pub async fn set_memory_limit(&self, limit: usize) {
        unsafe { self.lock().await.runtime.set_memory_limit(limit) }
    }

    /// Set a limit on the max size of stack the runtime will use.
    ///
    /// The default values is 256x1024 bytes.
    pub async fn set_max_stack_size(&self, limit: usize) {
        unsafe { self.lock().await.runtime.set_max_stack_size(limit) }
    }

    /// Set a memory threshold for garbage collection.
    pub async fn set_gc_threshold(&self, threshold: usize) {
        unsafe { self.lock().await.runtime.set_gc_threshold(threshold) }
    }

    /// Manually run the garbage collection.
    ///
    /// Most QuickJS values are reference counted and
    /// will automatically free themselves when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub async fn run_gc(&self) {
        let mut lock = self.lock().await;
        lock.drop_pending();
        unsafe { lock.runtime.run_gc() }
    }

    /// Get memory usage stats.
    pub async fn memory_usage(&self) -> MemoryUsage {
        unsafe { self.lock().await.runtime.memory_usage() }
    }

    /// Test for pending jobs.
    ///
    /// Returns true when at least one job is pending.
    pub async fn is_job_pending(&self) -> bool {
        let lock = self.lock().await;
        lock.runtime.is_job_pending() || !lock.runtime.get_opaque().spawner_is_empty()
    }

    /// Execute first pending job.
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    pub async fn execute_pending_job(&self) -> StdResult<bool, AsyncJobException> {
        let mut lock = self.lock().await;
        lock.runtime.update_stack_top();
        lock.drop_pending();

        if let Err(e) = lock.runtime.execute_pending_job() {
            let ptr = NonNull::new(e).expect("null context on error");
            return Err(AsyncJobException(unsafe {
                AsyncContext::from_raw(ptr, self.clone())
            }));
        }

        Ok(lock.runtime.is_job_pending() || !lock.runtime.get_opaque().spawner_is_empty())
    }

    /// Run all futures and jobs until finished.
    pub async fn idle(&self) {
        core::future::poll_fn(|cx| {
            let Some(mut lock) = self.try_lock() else {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            };

            lock.runtime.update_stack_top();
            lock.drop_pending();

            // Run all pending JS jobs
            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        let ctx = unsafe { Ctx::from_ptr(e) };
                        let err = ctx.catch();
                        #[cfg(feature = "std")]
                        {
                            use std::println;
                            if let Some(ex) = err
                                .clone()
                                .into_object()
                                .and_then(crate::Exception::from_object)
                            {
                                println!("error executing job: {}", ex);
                            } else {
                                println!("error executing job: {:?}", err);
                            }
                        }
                        let _ = err;
                    }
                }
            }

            match lock.runtime.get_opaque().poll(cx) {
                TaskPoll::Empty => Poll::Ready(()),
                TaskPoll::Progress => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                TaskPoll::Pending => Poll::Pending,
            }
        })
        .await
    }

    /// Returns a future that completes when the runtime is dropped.
    ///
    /// If the future is polled it will drive futures spawned inside the runtime completing them
    /// even if runtime is currently not in use.
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

        let rt = new_thread.enable_all().build().unwrap();

        #[cfg(feature = "parallel")]
        rt.block_on(async {
            let $rt = crate::AsyncRuntime::new().unwrap();
            let $ctx = crate::AsyncContext::full(&$rt).await.unwrap();
            $($t)*
        });
        #[cfg(not(feature = "parallel"))]
        {
            let set = tokio::task::LocalSet::new();
            set.block_on(&rt, async {
                let $rt = crate::AsyncRuntime::new().unwrap();
                let $ctx = crate::AsyncContext::full(&$rt).await.unwrap();
                $($t)*
            });
        }
    }
    };
}

#[cfg(test)]
mod test {
    use self::context::EvalOptions;
    use crate::*;
    use std::time::Duration;

    async_test_case!(basic => (_rt,ctx){
        async_with!(&ctx => |ctx|{
            let res: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(res,2i32);
        }).await;
    });

    async_test_case!(sleep_closure => (_rt,ctx){
        let mut a = 1;
        let a_ref = &mut a;

        async_with!(&ctx => |ctx|{
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

        tokio::time::sleep(Duration::from_secs_f64(0.01)).await;

        let number = Arc::new(AtomicUsize::new(0));
        let number_clone = number.clone();

        async_with!(&ctx => |ctx|{
            ctx.spawn(async move {
                tokio::task::yield_now().await;
                number_clone.store(1,Ordering::SeqCst);
            });
        }).await;
        assert_eq!(number.load(Ordering::SeqCst),0);
        tokio::time::sleep(Duration::from_secs_f64(0.01)).await;
        assert_eq!(number.load(Ordering::SeqCst),1);
    });

    async_test_case!(no_drive => (rt,ctx){
        use std::sync::{Arc, atomic::{Ordering,AtomicUsize}};

        let number = Arc::new(AtomicUsize::new(0));
        let number_clone = number.clone();

        async_with!(&ctx => |ctx|{
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

        async_with!(&ctx => |ctx|{
            ctx.spawn(async move {
                tokio::task::yield_now().await;
                number_clone.store(1,Ordering::SeqCst);
            });
        }).await;
        assert_eq!(number.load(Ordering::SeqCst),0);
        rt.idle().await;
        assert_eq!(number.load(Ordering::SeqCst),1);
    });

    async_test_case!(recursive_spawn => (rt,ctx){
        use tokio::sync::oneshot;

        async_with!(&ctx => |ctx|{
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
                for _ in 0..32 { ctx_clone.spawn(async move {}) }
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
            setTimeout(async () => { inc_count() }, 100);
          }, 100);
        }
        main().catch(print);
        "#;

        fn inc_count() { COUNT.fetch_add(1,Ordering::Relaxed); }

        fn set_timeout_spawn<'js>(ctx: Ctx<'js>, callback: Function<'js>, millis: usize) -> Result<()> {
            ctx.spawn(async move {
                tokio::time::sleep(Duration::from_millis(millis as u64)).await;
                callback.call::<_, ()>(()).unwrap();
            });
            Ok(())
        }

        async_with!(ctx => |ctx|{
            let res: Result<Promise> = (|| {
                let globals = ctx.globals();
                globals.set("inc_count", Func::from(inc_count))?;
                globals.set("setTimeout", Func::from(set_timeout_spawn))?;
                ctx.eval_with_options(SCRIPT, EvalOptions { promise: true, strict: false, ..Default::default() })
            })();

            match res.catch(&ctx) {
                Ok(promise) => { let _ = promise.into_future::<Value>().await.catch(&ctx); },
                Err(err) => { #[cfg(feature = "std")] std::println!("{}", err); },
            };
        }).await;

        rt.idle().await;
        assert_eq!(COUNT.load(Ordering::Relaxed), 2);
    });

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn ensure_types_are_send() {
        fn assert_send<T: Send>(_: &T) {}
        let rt = AsyncRuntime::new().unwrap();
        assert_send(&rt.idle());
        assert_send(&rt.execute_pending_job());
        assert_send(&rt.drive());
    }
}
