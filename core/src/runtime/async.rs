use std::{
    ffi::CString,
    ptr::NonNull,
    result::Result as StdResult,
    sync::{Arc, Weak},
};

use async_lock::Mutex;

#[cfg(feature = "allocator")]
use crate::allocator::Allocator;
#[cfg(feature = "loader")]
use crate::loader::{RawLoader, Resolver};
use crate::{
    context::AsyncContext, markers::ParallelSend, result::AsyncJobException, Ctx, Error, Exception,
    Result,
};

use super::{
    raw::{Opaque, RawRuntime},
    spawner::DriveFuture,
    InterruptHandler, MemoryUsage,
};

#[derive(Clone)]
pub struct AsyncWeakRuntime(Weak<Mutex<RawRuntime>>);

impl AsyncWeakRuntime {
    pub fn try_ref(&self) -> Option<AsyncRuntime> {
        self.0.upgrade().map(|inner| AsyncRuntime { inner })
    }
}

/// Asynchronous Quickjs runtime, entry point of the library.
#[derive(Clone)]
pub struct AsyncRuntime {
    // use Arc instead of Ref so we can use OwnedLock
    pub(crate) inner: Arc<Mutex<RawRuntime>>,
}

// Since all functions which use runtime are behind a mutex
// sending the runtime to other threads should be fine.
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncRuntime {}
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncWeakRuntime {}

// Since a global lock needs to be locked for safe use
// using runtime in a sync way should be safe as
// simultanious accesses is syncronized behind a lock.
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
    pub fn new() -> Result<Self> {
        let opaque = Opaque::with_spawner();
        let rt = unsafe { RawRuntime::new(opaque) }.ok_or(Error::Allocation)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(rt)),
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
        let opaque = Opaque::with_spawner();
        let rt = unsafe { RawRuntime::new_with_allocator(opaque, allocator) }
            .ok_or(Error::Allocation)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(rt)),
        })
    }

    /// Get weak ref to runtime
    pub fn weak(&self) -> AsyncWeakRuntime {
        AsyncWeakRuntime(Arc::downgrade(&self.inner))
    }

    /// Set a closure which is regularly called by the engine when it is executing code.
    /// If the provided closure returns `true` the interpreter will raise and uncatchable
    /// exception and return control flow to the caller.
    #[inline]
    pub async fn set_interrupt_handler(&self, handler: Option<InterruptHandler>) {
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
        let mut lock = self.inner.lock().await;

        lock.is_job_pending() || !unsafe { lock.get_opaque_mut().spawner() }.is_empty()
    }

    /// Execute first pending job
    ///
    /// Returns true when job was executed or false when queue is empty or error when exception thrown under execution.
    #[inline]
    pub async fn execute_pending_job(&self) -> StdResult<bool, AsyncJobException> {
        let mut lock = self.inner.lock().await;
        lock.update_stack_top();

        let job_res = lock.execute_pending_job().map_err(|e| {
            let ptr =
                NonNull::new(e).expect("executing pending job returned a null context on error");
            AsyncJobException(AsyncContext::from_raw(ptr, self.clone()))
        })?;
        if job_res {
            return Ok(true);
        }

        Ok(unsafe { lock.get_opaque_mut() }.spawner().drive().await)
    }

    /// Run all futures and jobs in the runtime until all are finished.
    #[inline]
    pub async fn idle(&self) {
        let mut lock = self.inner.lock().await;
        lock.update_stack_top();

        loop {
            match lock.execute_pending_job().map_err(|e| {
                let ptr = NonNull::new(e)
                    .expect("executing pending job returned a null context on error");
                AsyncJobException(AsyncContext::from_raw(ptr, self.clone()))
            }) {
                Err(e) => {
                    // SAFETY: Runtime is already locked so creating a context is safe.
                    let ctx = unsafe { Ctx::from_ptr(e.0.ctx.as_ptr()) };
                    let err = ctx.catch();
                    if let Some(x) = err.clone().into_object().and_then(Exception::from_object) {
                        // TODO do somthing better with errors.
                        println!("error executing job: {}", x);
                    } else {
                        println!("error executing job: {:?}", err);
                    }
                }
                Ok(true) => continue,
                Ok(false) => {}
            }

            if unsafe { lock.get_opaque_mut() }.spawner().drive().await {
                continue;
            }

            break;
        }
    }

    /// Returns a future that completes when the runtime is dropped.
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
        let rt = if cfg!(feature = "parallel") {
            tokio::runtime::Builder::new_multi_thread()
        } else {
            tokio::runtime::Builder::new_current_thread()
        }
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

        // Give drive time to start.
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
        // Give drive time to finish the task.
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
}
