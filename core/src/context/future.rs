use std::{
    future::Future,
    mem,
    pin::{pin, Pin},
    ptr::NonNull,
    task::{Context, Poll},
};

use crate::{
    intrinsic, qjs, runtime::Inner, ContextBuilder, Ctx, Error, Intrinsic, Result, Runtime,
};

pub(crate) struct WithFuture<'js, F> {
    pub future: Pin<&'js mut F>,
    pub runtime: &'js mut Inner,
}

impl<'js, F> Future for WithFuture<'js, F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = self.future.as_mut().poll(cx);

        loop {
            match self.runtime.execute_pending_job() {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    // TODO do something here.
                }
            }
        }

        while let Poll::Ready(true) = {
            let this = &mut pin!(unsafe { self.runtime.get_opaque_mut() }.pending.poll());
            Future::poll(Pin::new(this), cx)
        } {}
        res
    }
}

/// A single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
pub struct AsyncContext {
    ctx: NonNull<qjs::JSContext>,
    rt: Runtime,
}

impl Clone for AsyncContext {
    fn clone(&self) -> AsyncContext {
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(self.ctx.as_ptr())) };
        let rt = self.rt.clone();
        Self { ctx, rt }
    }
}

impl AsyncContext {
    pub fn from_ctx<'js>(ctx: Ctx<'js>) -> Result<Self> {
        let rt = unsafe { &ctx.get_opaque().runtime }
            .try_ref()
            .ok_or(Error::Unknown)?;
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(ctx.as_ptr())) };
        Ok(Self { ctx, rt })
    }

    /// Creates a base context with only the required functions registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub async fn base(runtime: &Runtime) -> Result<Self> {
        Self::custom::<intrinsic::Base>(runtime).await
    }

    /// Creates a context with only the required intrinsics registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub async fn custom<I: Intrinsic>(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.async_lock().await;
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
    /// [`Context::custom`] or [`Context::builder`].
    pub async fn full(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.async_lock().await;
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
        let guard = self.rt.inner.async_lock().await;
        guard.update_stack_top();
        unsafe { qjs::JS_EnableBignumExt(self.ctx.as_ptr(), i32::from(enable)) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard)
    }

    /// Returns the associated runtime
    pub fn runtime(&self) -> &Runtime {
        &self.rt
    }

    pub(crate) fn get_runtime_ptr(&self) -> *mut qjs::JSRuntime {
        unsafe { qjs::JS_GetRuntime(self.ctx.as_ptr()) }
    }

    #[cfg(feature = "futures")]
    pub async unsafe fn unsafe_async_with<F, Fut, R>(&self, f: F) -> R
    where
        F: FnOnce(NonNull<qjs::JSContext>) -> Fut,
        Fut: Future<Output = R>,
    {
        let mut guard = self.rt.inner.async_lock().await;
        guard.update_stack_top();
        let future = std::pin::pin!(f(self.ctx));
        WithFuture {
            future,
            runtime: &mut *guard,
        }
        .await
    }

    pub(crate) unsafe fn init_raw(ctx: *mut qjs::JSContext) {
        Runtime::init_raw(qjs::JS_GetRuntime(ctx));
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    #[test]
    fn base() {
        test_with(|ctx| {
            let val: Value = ctx.eval(r#"1+1"#).unwrap();

            assert_eq!(val.type_of(), Type::Int);
            assert_eq!(i32::from_js(ctx, val).unwrap(), 2);
            println!("{:?}", ctx.globals());
        });
    }

    #[test]
    fn minimal() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::builder()
            .with::<intrinsic::Eval>()
            .build(&rt)
            .unwrap();
        ctx.with(|ctx| {
            let val: i32 = ctx.eval(r#"1+1"#).unwrap();

            assert_eq!(val, 2);
            println!("{:?}", ctx.globals());
        });
    }

    #[cfg(feature = "exports")]
    #[test]
    fn module() {
        test_with(|ctx| {
            let _value: Module = ctx
                .compile(
                    "test_mod",
                    r#"
                    let t = "3";
                    let b = (a) => a + 3;
                    export { b, t}
                "#,
                )
                .unwrap();
        });
    }

    #[test]
    #[cfg(feature = "parallel")]
    fn parallel() {
        use std::thread;

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let _: () = ctx.eval("this.foo = 42").unwrap();
        });
        thread::spawn(move || {
            ctx.with(|ctx| {
                let i: i32 = ctx.eval("foo + 8").unwrap();
                assert_eq!(i, 50);
            });
        })
        .join()
        .unwrap();
    }

    #[test]
    #[should_panic(
        expected = "Exception generated by quickjs: [eval_script]:1 invalid first character of private name\n    at eval_script:1\n"
    )]
    fn exception() {
        test_with(|ctx| {
            let val = ctx.eval::<(), _>("bla?#@!@ ");
            if let Err(e) = val {
                assert!(e.is_exception());
                panic!("{}", e);
            } else {
                panic!();
            }
        });
    }
}
