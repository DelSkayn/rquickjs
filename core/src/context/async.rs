use std::{
    future::Future,
    mem,
    pin::{pin, Pin},
    ptr::NonNull,
    task::{ready, Context, Poll},
};

use async_lock::futures::LockArc;

use crate::{
    markers::ParallelSend,
    qjs,
    runtime::{raw::RawRuntime, AsyncRuntime},
    Ctx, Error, Result,
};

use super::{intrinsic, ContextBuilder, Intrinsic};

struct WithFuture<'js, R> {
    future: Pin<Box<dyn Future<Output = R> + 'js + Send>>,
    context: AsyncContext,
    lock: LockArc<RawRuntime>,
}

impl<'js, R> Future for WithFuture<'js, R> {
    type Output = R;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = ready!(pin!(&mut self.lock).poll(cx));
        lock.update_stack_top();
        let res = self.future.as_mut().poll(cx);
        unsafe {
            loop {
                if let Ok(true) = lock.execute_pending_job() {
                    continue;
                }

                let fut = pin!(lock.get_opaque_mut().spawner().drive());
                if let Poll::Ready(true) = fut.poll(cx) {
                    continue;
                }

                break;
            }
        }
        self.lock = self.context.rt.inner.lock_arc();
        res
    }
}

//#[cfg(feature = "parallel")]
unsafe impl<R> Send for WithFuture<'_, R> {}

/// A macro for safely using an asynchronous context while capturing the environment.
///
/// # Usage
/// ```
/// # use rquickjs::{prelude::*, Function, async_with, AsyncRuntime, AsyncContext, Result};
/// # use std::time::Duration;
/// # async fn run(){
/// let rt = AsyncRuntime::new().unwrap();
/// let ctx = AsyncContext::full(&rt).await.unwrap();
///
/// // In order for futures to conver to javascript promises they need to return `Result`.
/// async fn delay<'js>(amount: f64, cb: Function<'js>) -> Result<()> {
///     tokio::time::sleep(Duration::from_secs_f64(amount)).await;
///     cb.call::<(), ()>(());
///     Ok(())
/// }
///
/// fn print(text: String) -> Result<()> {
///     println!("{}", text);
///     Ok(())
/// }
///
/// let mut some_var = 1;
/// // closure always moves, so create a ref.
/// let some_var_ref = &mut some_var;
/// async_with!(ctx => |ctx|{
///     
///     // With the macro you can borrow the environment.
///     *some_var_ref += 1;
///
///     let delay = Func::new("delay", Async(delay));
///
///     let global = ctx.globals();
///     global.set("print",Func::from(print)).unwrap();
///     global.set("delay",delay).unwrap();
///     ctx.eval::<(),_>(r#"
///         print("start");
///         delay(1,() => {
///             print("delayed");
///         })
///         print("after");
///     "#).unwrap();
/// }).await;
/// assert_eq!(some_var,2);
///
/// rt.idle().await
/// # }
/// ```
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[macro_export]
macro_rules! async_with{
    ($context:expr => |$ctx:ident| { $($t:tt)* }) => {
        $crate::AsyncContext::async_with(&$context,|$ctx| {
            let fut = Box::pin(async move {
                $($t)*
            });
            /// SAFETY: While rquickjs objects have a 'js lifetime attached to them,
            /// they actually life much longer an the lifetime is just for checking
            /// if they belong to the correct context.
            /// By requiring that everything is moved into the closure outside
            /// environments still can't life shorter than the closure.
            /// This allows use to recast the future to a higher lifetime without problems.
            /// Second, the future will always aquire a lock before running. The closure
            /// enforces that everything moved into the future is send, but non of the
            /// rquickjs objects are send so the future will never be send.
            /// Since we aquire a lock before running the future and nothing can escape the closure
            /// and future it is safe to recast the future as send.
            unsafe fn uplift<'a,'b,R>(f: std::pin::Pin<Box<dyn std::future::Future<Output = R> + 'a>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = R> + 'b + Send>>{
                std::mem::transmute(f)
            }
            unsafe{ uplift(fut) }
        })
    };
}

/// An asynchronous single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct AsyncContext {
    pub(crate) ctx: NonNull<qjs::JSContext>,
    pub(crate) rt: AsyncRuntime,
}

impl Clone for AsyncContext {
    fn clone(&self) -> AsyncContext {
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(self.ctx.as_ptr())) };
        let rt = self.rt.clone();
        Self { ctx, rt }
    }
}

impl AsyncContext {
    pub(crate) fn from_raw(ctx: NonNull<qjs::JSContext>, rt: AsyncRuntime) -> Self {
        AsyncContext { ctx, rt }
    }

    /// Creates a base context with only the required functions registered.
    /// If additional functions are required use [`AsyncContext::custom`],
    /// [`AsyncContext::builder`] or [`AsyncContext::full`].
    pub async fn base(runtime: &AsyncRuntime) -> Result<Self> {
        Self::custom::<intrinsic::Base>(runtime).await
    }

    /// Creates a context with only the required intrinsics registered.
    /// If additional functions are required use [`AsyncContext::custom`],
    /// [`AsyncContext::builder`] or [`AsyncContext::full`].
    pub async fn custom<I: Intrinsic>(runtime: &AsyncRuntime) -> Result<Self> {
        let guard = runtime.inner.lock().await;
        let ctx = NonNull::new(unsafe { qjs::JS_NewContextRaw(guard.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        unsafe { I::add_intrinsic(ctx) };
        let res = AsyncContext {
            ctx,
            rt: runtime.clone(),
        };
        mem::drop(guard);

        Ok(res)
    }

    /// Creates a context with all standart available intrinsics registered.
    /// If precise controll is required of which functions are available use
    /// [`AsyncContext::custom`] or [`AsyncContext::builder`].
    pub async fn full(runtime: &AsyncRuntime) -> Result<Self> {
        let guard = runtime.inner.lock().await;
        let ctx = NonNull::new(unsafe { qjs::JS_NewContext(guard.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        let res = AsyncContext {
            ctx,
            rt: runtime.clone(),
        };
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);

        Ok(res)
    }

    /// Create a context builder for creating a context with a specific set of intrinsics
    pub fn builder() -> ContextBuilder<()> {
        ContextBuilder::default()
    }

    pub async fn enable_big_num_ext(&self, enable: bool) {
        let guard = self.rt.inner.lock().await;
        guard.update_stack_top();
        unsafe { qjs::JS_EnableBignumExt(self.ctx.as_ptr(), i32::from(enable)) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard)
    }

    /// Returns the associated runtime
    pub fn runtime(&self) -> &AsyncRuntime {
        &self.rt
    }

    /// A entry point for manipulating and using javascript objects and scripts.
    ///
    /// This function is rather limited in what environment it can capture. If you need to borrow
    /// the environment in the closure use the [`async_with!`] macro.
    ///
    /// Unfortunatly it is currently impossible to have closures return a generic future which has a higher
    /// rank trait bound lifetime. So, to allow closures to work, the closure must return a boxed
    /// future.
    pub async fn async_with<F, R>(&self, f: F) -> R
    where
        F: for<'js> FnOnce(Ctx<'js>) -> Pin<Box<dyn Future<Output = R> + 'js + Send>>
            + ParallelSend,
        R: ParallelSend,
    {
        let future = {
            let guard = self.rt.inner.lock().await;
            guard.update_stack_top();
            let ctx = unsafe { Ctx::new_async(self) };
            f(ctx)
        };
        WithFuture {
            future,
            context: self.clone(),
            lock: self.rt.inner.lock_arc(),
        }
        .await
    }

    /// A entry point for manipulating and using javascript objects and scripts.
    ///
    /// This closure can't return a future, if you need to await javascript promises prefer the
    /// [`async_with!`] macro.
    pub async fn with<F, R>(&self, f: F) -> R
    where
        F: for<'js> FnOnce(Ctx<'js>) -> R + ParallelSend,
        R: ParallelSend,
    {
        let guard = self.rt.inner.lock().await;
        guard.update_stack_top();
        let ctx = unsafe { Ctx::new_async(self) };
        f(ctx)
    }
}

impl Drop for AsyncContext {
    fn drop(&mut self) {
        //TODO
        let guard = match self.rt.inner.try_lock() {
            Some(x) => x,
            None => {
                let p = unsafe { &mut *(self.ctx.as_ptr() as *mut qjs::JSRefCountHeader) };
                if p.ref_count <= 1 {
                    // Lock was poisened, this should only happen on a panic.
                    // We should still free the context.
                    // TODO see if there is a way to recover from a panic which could cause the
                    // following assertion to trigger
                    assert!(std::thread::panicking());
                }
                unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) }
                return;
            }
        };
        guard.update_stack_top();
        unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }
}

// Since the reference to runtime is behind a Arc this object is send
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncContext {}

// Since all functions lock the global runtime lock access is synchronized so
// this object is sync
#[cfg(feature = "parallel")]
unsafe impl Sync for AsyncContext {}
