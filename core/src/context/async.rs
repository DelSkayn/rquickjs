use std::{
    future::Future,
    mem,
    pin::{pin, Pin},
    ptr::NonNull,
    task::{Context, Poll},
};

use crate::{
    markers::ParallelSend,
    qjs,
    runtime::{raw::RawRuntime, AsyncRuntime},
    Error, Result,
};

use super::{intrinsic, ContextBuilder, Intrinsic};

struct WithFuture<'js, F> {
    future: Pin<&'js mut F>,
    runtime: &'js mut RawRuntime,
}

impl<'js, F> Future for WithFuture<'js, F>
where
    F: Future,
{
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let res = self.future.as_mut().poll(cx);

            loop {
                let fut = pin!(self.runtime.get_opaque_mut().spawner().drive());
                if let Poll::Ready(true) = fut.poll(cx) {
                    continue;
                }

                if let Ok(true) = self.runtime.execute_pending_job() {
                    continue;
                }

                break;
            }
            res
        }
    }
}

/// A macro for safely using an asynchronous context.
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
/// async_with!(ctx => |ctx|{
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
///
/// rt.idle().await
/// # }
/// ```

#[macro_export]
macro_rules! async_with{
    ($context:expr => |$ctx:ident|{ $($t:tt)* }) => {
        unsafe{
        $crate::AsyncContext::async_with(&$context,|ctx| async move {
               let _pin = ();
                let invariant = $crate::markers::Invariant::new_ref(&_pin);
                let _lifetime_constrainer;
                if false {
                    struct KeepTillScopeDrop<'a, 'inv>(&'a $crate::markers::Invariant<'inv>);
                    impl<'a, 'inv> Drop for KeepTillScopeDrop<'a, 'inv> {
                        fn drop(&mut self) {}
                    }
                    _lifetime_constrainer = KeepTillScopeDrop(&invariant);
                }
                let $ctx = $crate::Ctx::from_ptr_invariant(ctx,invariant);
                {
                $($t)*
                }
        })
        }
    };
}

/// An asynchronous single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
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
    /// The api is structured this way to avoid repeated locking the runtime when ever
    /// any function is called. This way the runtime is locked once before executing the callback.
    /// Furthermore, this way it is impossible to use values from different runtimes in this
    /// context which would otherwise be undefined behaviour.
    ///
    /// For async contexts this function returns a pointer of which usage is unsafe, use the
    /// [`async_with`] macro to safely use async contexts.
    ///
    pub async fn async_with<F, Fut, R>(&self, f: F) -> R
    where
        F: FnOnce(NonNull<qjs::JSContext>) -> Fut + ParallelSend,
        Fut: Future<Output = R>,
    {
        let mut guard = self.rt.inner.lock().await;
        guard.update_stack_top();
        let future = std::pin::pin!(f(self.ctx));
        WithFuture {
            future,
            runtime: &mut guard,
        }
        .await
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
//
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncContext {}

// Since all functions lock the global runtime lock access is synchronized so
// this object is sync
#[cfg(feature = "parallel")]
unsafe impl Sync for AsyncContext {}
