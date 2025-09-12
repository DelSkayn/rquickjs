use super::{
    ctx::RefCountHeader,
    intrinsic,
    owner::{ContextOwner, DropContext},
    ContextBuilder, Intrinsic,
};
use crate::{qjs, Ctx, Error, Result, Runtime};
use core::{mem, ptr::NonNull};

impl DropContext for Runtime {
    unsafe fn drop_context(&self, ctx: NonNull<qjs::JSContext>) {
        //TODO
        let guard = match self.inner.try_lock() {
            Some(x) => x,
            None => {
                let p = unsafe { &mut *(ctx.as_ptr() as *mut RefCountHeader) };
                if p.ref_count <= 1 {
                    // Lock was poisoned, this should only happen on a panic.
                    // We should still free the context.
                    // TODO see if there is a way to recover from a panic which could cause the
                    // following assertion to trigger
                    #[cfg(feature = "std")]
                    assert!(std::thread::panicking());
                }
                unsafe { qjs::JS_FreeContext(ctx.as_ptr()) }
                return;
            }
        };
        guard.update_stack_top();
        unsafe { qjs::JS_FreeContext(ctx.as_ptr()) }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }
}

/// A single execution context with its own global variables and stack.
///
/// Can share objects with other contexts of the same runtime.
#[derive(Clone)]
pub struct Context(pub(crate) ContextOwner<Runtime>);

impl Context {
    /// Create a unused context from a raw context pointer.
    ///
    /// # Safety
    /// Pointer must point to a context from the given runtime.
    /// The context must also have valid reference count, one which can be decremented when this
    /// object is dropped without going negative.
    pub unsafe fn from_raw(ctx: NonNull<qjs::JSContext>, rt: Runtime) -> Self {
        Context(ContextOwner::new(ctx, rt))
    }

    pub fn as_raw(&self) -> NonNull<qjs::JSContext> {
        self.0.ctx()
    }

    /// Creates a base context with only the required functions registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub fn base(runtime: &Runtime) -> Result<Self> {
        Self::custom::<intrinsic::None>(runtime)
    }

    /// Creates a context with only the required intrinsics registered.
    /// If additional functions are required use [`Context::custom`],
    /// [`Context::builder`] or [`Context::full`].
    pub fn custom<I: Intrinsic>(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = NonNull::new(unsafe { qjs::JS_NewContextRaw(guard.rt.as_ptr()) })
            .ok_or(Error::Allocation)?;
        // rquickjs assumes the base objects exist, so we allways need to add this.
        unsafe { qjs::JS_AddIntrinsicBaseObjects(ctx.as_ptr()) };
        unsafe { I::add_intrinsic(ctx) };
        let res = unsafe { ContextOwner::new(ctx, runtime.clone()) };
        mem::drop(guard);

        Ok(Context(res))
    }

    /// Creates a context with all standard available intrinsics registered.
    /// If precise control is required of which functions are available use
    /// [`Context::custom`] or [`Context::builder`].
    pub fn full(runtime: &Runtime) -> Result<Self> {
        let guard = runtime.inner.lock();
        let ctx = NonNull::new(unsafe { qjs::JS_NewContext(guard.rt.as_ptr()) })
            .ok_or(Error::Allocation)?;
        let res = unsafe { ContextOwner::new(ctx, runtime.clone()) };
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);

        Ok(Context(res))
    }

    /// Create a context builder for creating a context with a specific set of intrinsics
    pub fn builder() -> ContextBuilder<()> {
        ContextBuilder::default()
    }

    /// Returns the associated runtime
    pub fn runtime(&self) -> &Runtime {
        self.0.rt()
    }

    #[allow(dead_code)]
    pub fn get_runtime_ptr(&self) -> *mut qjs::JSRuntime {
        unsafe { qjs::JS_GetRuntime(self.0.ctx().as_ptr()) }
    }

    /// A entry point for manipulating and using JavaScript objects and scripts.
    /// The api is structured this way to avoid repeated locking the runtime when ever
    /// any function is called. This way the runtime is locked once before executing the callback.
    /// Furthermore, this way it is impossible to use values from different runtimes in this
    /// context which would otherwise be undefined behavior.
    ///
    ///
    /// This is the only way to get a [`Ctx`] object.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx) -> R,
    {
        let guard = self.0.rt().inner.lock();
        guard.update_stack_top();
        let ctx = unsafe { Ctx::new(self) };
        f(ctx)
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
    fn basic() {
        test_with(|ctx| {
            let val: Value = ctx.eval(r#"1+1"#).unwrap();

            assert_eq!(val.type_of(), Type::Int);
            assert_eq!(i32::from_js(&ctx, val).unwrap(), 2);
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

    #[test]
    fn base() {
        let rt = Runtime::new().unwrap();
        let _ = Context::base(&rt).unwrap();
    }

    #[test]
    fn module() {
        test_with(|ctx| {
            Module::evaluate(
                ctx,
                "test_mod",
                r#"
                    let t = "3";
                    let b = (a) => a + 3;
                    export { b, t}
                "#,
            )
            .unwrap()
            .finish::<()>()
            .unwrap();
        });
    }

    #[test]
    fn clone_ctx() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::builder()
            .with::<intrinsic::Eval>()
            .build(&rt)
            .unwrap();

        let ctx_clone = ctx.clone();

        ctx.with(|ctx| {
            let val: i32 = ctx.eval(r#"1+1"#).unwrap();

            assert_eq!(val, 2);
            println!("{:?}", ctx.globals());
        });

        ctx_clone.with(|ctx| {
            let val: i32 = ctx.eval(r#"1+1"#).unwrap();

            assert_eq!(val, 2);
            println!("{:?}", ctx.globals());
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
    #[cfg(feature = "parallel")]
    fn parallel_drop() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let wait_for_entry = Arc::new(Barrier::new(2));

        let rt = Runtime::new().unwrap();
        let ctx_1 = Context::full(&rt).unwrap();
        let ctx_2 = Context::full(&rt).unwrap();
        let wait_for_entry_c = wait_for_entry.clone();
        thread::spawn(move || {
            wait_for_entry_c.wait();
            std::mem::drop(ctx_1);
            println!("done");
        });

        ctx_2.with(|ctx| {
            wait_for_entry.wait();
            let i: i32 = ctx.eval("2 + 8").unwrap();
            assert_eq!(i, 10);
        });
        println!("done");
    }

    // Will be improved by https://github.com/quickjs-ng/quickjs/pull/406
    #[test]
    #[should_panic(
        expected = "Error: invalid first character of private name\n    at eval_script:1:1\n"
    )]
    fn exception() {
        test_with(|ctx| {
            let val = ctx.eval::<(), _>("bla?#@!@ ").catch(&ctx);
            if let Err(e) = val {
                assert!(e.is_exception());
                panic!("{}", e);
            }
        });
    }
}
