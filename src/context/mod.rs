use crate::{Error, Result, Runtime};
use rquickjs_sys as qjs;
use std::mem;

mod builder;
pub use builder::ContextBuilder;
mod ctx;
pub use ctx::Ctx;
mod multi_with_impl;

/// A trait for using multiple contexts at the same time.
pub trait MultiWith<'js> {
    type Arg;

    /// Use multiple contexts together.
    ///
    /// # Panic
    /// This function will panic if any of the contexts are of seperate runtimes.
    fn with<R, F: FnOnce(Self::Arg) -> R>(self, f: F) -> R;
}

/// A single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
pub struct Context {
    //TODO replace with NotNull?
    pub(crate) ctx: *mut qjs::JSContext,
    rt: Runtime,
}

impl Context {
    /// Creates a base context with only the required functions registered.
    /// If additional functions are required use [`Context::build`](#method.build)
    /// or [`Contex::full`](#method.full).
    pub fn base(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = unsafe { qjs::JS_NewContextRaw(guard.rt) };
        if ctx.is_null() {
            return Err(Error::Allocation);
        }
        unsafe { qjs::JS_AddIntrinsicBaseObjects(ctx) };
        let res = Ok(Context {
            ctx,
            rt: runtime.clone(),
        });
        mem::drop(guard);
        res
    }

    /// Creates a context with all standart available functions registered.
    /// If precise controll is required of wich functions are availble use
    /// [`Context::build`](#method.context)
    pub fn full(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = unsafe { qjs::JS_NewContext(guard.rt) };
        if ctx.is_null() {
            return Err(Error::Allocation);
        }
        let res = Ok(Context {
            ctx,
            rt: runtime.clone(),
        });
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
        res
    }

    #[cfg(feature = "parallel")]
    fn reset_stack(&self) {
        unsafe {
            qjs::JS_ResetCtxStack(self.ctx);
        }
    }

    #[cfg(not(feature = "parallel"))]
    fn reset_stack(&self) {}

    /// Create a context builder for creating a context with a specific set of intrinsics
    pub fn build(rt: &Runtime) -> ContextBuilder {
        ContextBuilder::new(rt)
    }

    /// Set the maximum stack size for the local context stack
    pub fn set_max_stack_size(&self, size: usize) {
        let guard = self.rt.inner.lock();
        self.reset_stack();
        unsafe { qjs::JS_SetMaxStackSize(guard.rt, size as u64) };
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard)
    }

    pub fn enable_big_num_ext(&self, enable: bool) {
        let guard = self.rt.inner.lock();
        self.reset_stack();
        unsafe { qjs::JS_EnableBignumExt(self.ctx, if enable { 1 } else { 0 }) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard)
    }

    pub fn runtime(&self) -> &Runtime {
        &self.rt
    }

    pub(crate) fn get_runtime_ptr(&self) -> *mut qjs::JSRuntime {
        unsafe { qjs::JS_GetRuntime(self.ctx) }
    }

    /// A entry point for manipulating and using javascript objects and scripts.
    /// The api is structured this way to avoid repeated locking the runtime when ever
    /// any function is called. This way the runtime is locked once before executing the callback.
    /// Furthermore, this way it is impossible to use values from different runtimes in this
    /// context which would otherwise be undefined behaviour.
    ///
    ///
    /// This is the only way to get a [`Ctx`](struct.Ctx.html) object.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx) -> R,
    {
        let guard = self.rt.inner.lock();
        self.reset_stack();
        let ctx = Ctx::new(self);
        let res = f(ctx);
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
        res
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        //TODO
        let guard = match self.rt.inner.try_lock() {
            Ok(x) => x,
            Err(x) => {
                // Lock was poisened, this should only happen on a panic.
                // We should still free the context.
                // TODO see if there is a way to recover from a panic which could cause the
                // following assertion to trigger
                assert!(std::thread::panicking());
                x
            }
        };
        self.reset_stack();
        unsafe { qjs::JS_FreeContext(self.ctx) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard)
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
    use crate::{Module, Value};
    #[test]
    fn base() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(r#"1+1"#);

            assert_eq!(val.unwrap(), Value::Int(2));
            println!("{:?}", ctx.globals());
        });
    }

    #[test]
    fn minimal() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::build(&rt).none().eval(true).build().unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(r#"1+1"#);

            assert_eq!(val.unwrap(), Value::Int(2));
            println!("{:?}", ctx.globals());
        });
    }

    #[test]
    fn module() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
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
            ctx.eval::<(), _>("this.foo = 42").unwrap();
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
    fn exception() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<(), _>("bla?#@!@ ");
            if let Err(e) = val {
                assert!(e.is_exception());
                assert_eq!(format!("{}", e), "exception generated by quickjs: [eval_script]:1 invalid first character of private name\n    at eval_script:1\n".to_string());
            } else {
                panic!();
            }
        });
    }
}
