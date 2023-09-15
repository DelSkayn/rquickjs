use std::{future::Future, mem, pin::Pin, ptr::NonNull};

use crate::{markers::ParallelSend, qjs, runtime::AsyncRuntime, Ctx, Error, Result};

use self::future::WithFuture;

use super::{intrinsic, r#ref::ContextRef, ContextBuilder, Intrinsic};

mod future;

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
/// // In order for futures to convert to JavaScript promises they need to return `Result`.
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
///     let delay = Function::new(ctx.clone(),Async(delay))
///         .unwrap()
///         .with_name("print")
///         .unwrap();
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
            /// Second, the future will always acquire a lock before running. The closure
            /// enforces that everything moved into the future is send, but non of the
            /// rquickjs objects are send so the future will never be send.
            /// Since we acquire a lock before running the future and nothing can escape the closure
            /// and future it is safe to recast the future as send.
            unsafe fn uplift<'a,'b,R>(f: std::pin::Pin<Box<dyn std::future::Future<Output = R> + 'a>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = R> + 'b + Send>>{
                std::mem::transmute(f)
            }
            unsafe{ uplift(fut) }
        })
    };
}

pub(crate) struct Inner {
    pub(crate) ctx: NonNull<qjs::JSContext>,
    pub(crate) rt: AsyncRuntime,
}

impl Clone for Inner {
    fn clone(&self) -> Inner {
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(self.ctx.as_ptr())) };
        let rt = self.rt.clone();
        Self { ctx, rt }
    }
}

#[cfg(feature = "parallel")]
unsafe impl Send for Inner {}

impl Drop for Inner {
    fn drop(&mut self) {
        //TODO
        let guard = match self.rt.inner.try_lock() {
            Some(x) => x,
            None => {
                #[cfg(not(feature = "parallel"))]
                {
                    let p = unsafe { &mut *(self.ctx.as_ptr() as *mut qjs::JSRefCountHeader) };
                    if p.ref_count <= 1 {
                        // Lock was poisoned, this should only happen on a panic.
                        // We should still free the context.
                        // TODO see if there is a way to recover from a panic which could cause the
                        // following assertion to trigger
                        assert!(std::thread::panicking());
                    }
                    unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) }
                    return;
                }
                #[cfg(feature = "parallel")]
                {
                    self.rt
                        .drop_send
                        .send(self.ctx)
                        .expect("runtime should be alive while contexts life");
                    return;
                }
            }
        };
        guard.runtime.update_stack_top();
        unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }
}

/// An asynchronous single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[derive(Clone)]
pub struct AsyncContext(pub(crate) ContextRef<Inner>);

impl AsyncContext {
    /// Create a async context form a raw context pointer.
    ///
    /// # Safety
    /// The context must be of the correct runtime.
    /// The context must also have valid reference count, one which can be decremented when this
    /// object is dropped without going negative.
    pub unsafe fn from_raw(ctx: NonNull<qjs::JSContext>, rt: AsyncRuntime) -> Self {
        AsyncContext(ContextRef::new(Inner { ctx, rt }))
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
        let ctx = NonNull::new(unsafe { qjs::JS_NewContextRaw(guard.runtime.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        unsafe { I::add_intrinsic(ctx) };
        let res = Inner {
            ctx,
            rt: runtime.clone(),
        };
        guard.drop_pending();
        mem::drop(guard);

        Ok(AsyncContext(ContextRef::new(res)))
    }

    /// Creates a context with all standard available intrinsics registered.
    /// If precise control is required of which functions are available use
    /// [`AsyncContext::custom`] or [`AsyncContext::builder`].
    pub async fn full(runtime: &AsyncRuntime) -> Result<Self> {
        let guard = runtime.inner.lock().await;
        let ctx = NonNull::new(unsafe { qjs::JS_NewContext(guard.runtime.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        let res = Inner {
            ctx,
            rt: runtime.clone(),
        };
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        guard.drop_pending();
        mem::drop(guard);

        Ok(AsyncContext(ContextRef::new(res)))
    }

    /// Create a context builder for creating a context with a specific set of intrinsics
    pub fn builder() -> ContextBuilder<()> {
        ContextBuilder::default()
    }

    pub async fn enable_big_num_ext(&self, enable: bool) {
        let guard = self.0.rt.inner.lock().await;
        guard.runtime.update_stack_top();
        unsafe { qjs::JS_EnableBignumExt(self.0.ctx.as_ptr(), i32::from(enable)) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        guard.drop_pending();
        mem::drop(guard)
    }

    /// Returns the associated runtime
    pub fn runtime(&self) -> &AsyncRuntime {
        &self.0.rt
    }

    /// A entry point for manipulating and using JavaScript objects and scripts.
    ///
    /// This function is rather limited in what environment it can capture. If you need to borrow
    /// the environment in the closure use the [`async_with!`] macro.
    ///
    /// Unfortunately it is currently impossible to have closures return a generic future which has a higher
    /// rank trait bound lifetime. So, to allow closures to work, the closure must return a boxed
    /// future.
    pub fn async_with<F, R>(&self, f: F) -> WithFuture<F, R>
    where
        F: for<'js> FnOnce(Ctx<'js>) -> Pin<Box<dyn Future<Output = R> + 'js + Send>>
            + ParallelSend,
        R: ParallelSend,
    {
        WithFuture::new(self, f)
    }

    /// A entry point for manipulating and using JavaScript objects and scripts.
    ///
    /// This closure can't return a future, if you need to await JavaScript promises prefer the
    /// [`async_with!`] macro.
    pub async fn with<F, R>(&self, f: F) -> R
    where
        F: for<'js> FnOnce(Ctx<'js>) -> R + ParallelSend,
        R: ParallelSend,
    {
        let guard = self.0.rt.inner.lock().await;
        guard.runtime.update_stack_top();
        let ctx = unsafe { Ctx::new_async(self) };
        let res = f(ctx);
        guard.drop_pending();
        res
    }
}

// Since the reference to runtime is behind a Arc this object is send
#[cfg(feature = "parallel")]
unsafe impl Send for AsyncContext {}

// Since all functions lock the global runtime lock access is synchronized so
// this object is sync
#[cfg(feature = "parallel")]
unsafe impl Sync for AsyncContext {}

#[cfg(test)]
mod test {
    #[cfg(feature = "parallel")]
    use crate::{AsyncContext, AsyncRuntime};
    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn parallel_drop() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let wait_for_entry = Arc::new(Barrier::new(2));
        let wait_for_exit = Arc::new(Barrier::new(2));

        let rt = AsyncRuntime::new().unwrap();
        let ctx_1 = AsyncContext::full(&rt).await.unwrap();
        let ctx_2 = AsyncContext::full(&rt).await.unwrap();
        let wait_for_entry_c = wait_for_entry.clone();
        let wait_for_exit_c = wait_for_exit.clone();
        thread::spawn(move || {
            println!("wait_for entry ctx_1");
            wait_for_entry_c.wait();
            println!("dropping");
            std::mem::drop(ctx_1);
            println!("wait_for exit ctx_1");
            wait_for_exit_c.wait();
        });

        println!("wait_for entry ctx_2");
        rt.run_gc().await;
        ctx_2
            .with(|ctx| {
                wait_for_entry.wait();
                println!("evaling");
                let i: i32 = ctx.eval("2 + 8").unwrap();
                assert_eq!(i, 10);
                println!("wait_for exit ctx_2");
                wait_for_exit.wait();
            })
            .await;
    }
}
