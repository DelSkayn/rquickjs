use std::{mem, ptr::NonNull};

use crate::{qjs, runtime::raw::RawRuntime, Ctx, Error, Result, Runtime};

use super::{intrinsic, ContextBuilder, Intrinsic};

/// A single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
pub struct Context {
    pub(crate) ctx: NonNull<qjs::JSContext>,
    pub(crate) rt: Runtime,
}

impl Clone for Context {
    fn clone(&self) -> Context {
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(self.ctx.as_ptr())) };
        let rt = self.rt.clone();
        Self { ctx, rt }
    }
}

impl Context {
    pub(crate) fn from_raw(ctx: NonNull<qjs::JSContext>, rt: Runtime) -> Self {
        Context { ctx, rt }
    }

    /// Creates a base context with only the required functions registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub fn base(runtime: &Runtime) -> Result<Self> {
        Self::custom::<intrinsic::Base>(runtime)
    }

    /// Creates a context with only the required intrinsics registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub fn custom<I: Intrinsic>(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = NonNull::new(unsafe { qjs::JS_NewContextRaw(guard.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        unsafe { I::add_intrinsic(ctx) };
        let res = Context {
            ctx,
            rt: runtime.clone(),
        };
        mem::drop(guard);

        Ok(res)
    }

    /// Creates a context with all standart available intrinsics registered.
    /// If precise controll is required of which functions are available use
    /// [`Context::custom`] or [`Context::builder`].
    pub fn full(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = NonNull::new(unsafe { qjs::JS_NewContext(guard.rt.as_ptr()) })
            .ok_or_else(|| Error::Allocation)?;
        let res = Context {
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

    pub fn enable_big_num_ext(&self, enable: bool) {
        let guard = self.rt.inner.lock();
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

    /// A entry point for manipulating and using javascript objects and scripts.
    /// The api is structured this way to avoid repeated locking the runtime when ever
    /// any function is called. This way the runtime is locked once before executing the callback.
    /// Furthermore, this way it is impossible to use values from different runtimes in this
    /// context which would otherwise be undefined behaviour.
    ///
    ///
    /// This is the only way to get a [`Ctx`] object.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx) -> R,
    {
        let guard = self.rt.inner.lock();
        guard.update_stack_top();
        let ctx = unsafe { Ctx::new(self) };
        f(ctx)
    }

    pub(crate) unsafe fn init_raw(ctx: *mut qjs::JSContext) {
        RawRuntime::init_raw(qjs::JS_GetRuntime(ctx));
    }
}

impl Drop for Context {
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
unsafe impl Send for Context {}

// Since all functions lock the global runtime lock access is synchronized so
// this object is sync
#[cfg(feature = "parallel")]
unsafe impl Sync for Context {}

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
            let val = ctx.eval::<(), _>("bla?#@!@ ").catch(ctx);
            if let Err(e) = val {
                assert!(e.is_exception());
                panic!("{}", e);
            }
        });
    }
}
