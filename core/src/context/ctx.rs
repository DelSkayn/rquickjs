use crate::{
    markers::Invariant, qjs, runtime::Opaque, Context, FromJs, Function, Module, Object, Result,
    Value,
};

#[cfg(feature = "futures")]
use std::future::Future;

#[cfg(feature = "futures")]
use crate::ParallelSend;

use std::{
    ffi::{CStr, CString},
    fs,
    marker::PhantomData,
    mem,
    path::Path,
    ptr::NonNull,
};

/// Eval options.
pub struct EvalOptions {
    /// Global code.
    pub global: bool,
    /// Force 'strict' mode.
    pub strict: bool,
    /// Don't include the stack frames before this eval in the Error() backtraces.
    pub backtrace_barrier: bool,
}

impl EvalOptions {
    fn to_flag(&self) -> i32 {
        let mut flag = if self.global {
            qjs::JS_EVAL_TYPE_GLOBAL
        } else {
            qjs::JS_EVAL_TYPE_MODULE
        };

        if self.strict {
            flag |= qjs::JS_EVAL_FLAG_STRICT;
        }

        if self.backtrace_barrier {
            flag |= qjs::JS_EVAL_FLAG_BACKTRACE_BARRIER;
        }

        flag as i32
    }
}

impl Default for EvalOptions {
    fn default() -> Self {
        EvalOptions {
            global: true,
            strict: true,
            backtrace_barrier: false,
        }
    }
}

/// Context in use, passed to [`Context::with`].
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'js> {
    ctx: NonNull<qjs::JSContext>,
    marker: Invariant<'js>,
}

impl<'js> Ctx<'js> {
    pub(crate) fn as_ptr(&self) -> *mut qjs::JSContext {
        self.ctx.as_ptr()
    }

    pub(crate) unsafe fn from_ptr(ctx: *mut qjs::JSContext) -> Self {
        let ctx = NonNull::new_unchecked(ctx);
        Ctx {
            ctx,
            marker: PhantomData,
        }
    }

    pub(crate) fn new(ctx: &'js Context) -> Self {
        Ctx {
            ctx: ctx.ctx,
            marker: PhantomData,
        }
    }

    pub(crate) unsafe fn eval_raw<S: Into<Vec<u8>>>(
        self,
        source: S,
        file_name: &CStr,
        flag: i32,
    ) -> Result<qjs::JSValue> {
        let src = source.into();
        let len = src.len();
        let src = CString::new(src)?;
        let val = qjs::JS_Eval(
            self.ctx.as_ptr(),
            src.as_ptr(),
            len as _,
            file_name.as_ptr(),
            flag,
        );
        self.handle_exception(val)
    }

    /// Evaluate a script in global context.
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(self, source: S) -> Result<V> {
        self.eval_with_options(source, Default::default())
    }

    /// Evaluate a script with the given options.
    pub fn eval_with_options<V: FromJs<'js>, S: Into<Vec<u8>>>(
        self,
        source: S,
        options: EvalOptions,
    ) -> Result<V> {
        let file_name = cstr!("eval_script");

        V::from_js(self, unsafe {
            let val = self.eval_raw(source, file_name, options.to_flag())?;
            Value::from_js_value(self, val)
        })
    }

    /// Evaluate a script directly from a file.
    pub fn eval_file<V: FromJs<'js>, P: AsRef<Path>>(self, path: P) -> Result<V> {
        self.eval_file_with_options(path, Default::default())
    }

    pub fn eval_file_with_options<V: FromJs<'js>, P: AsRef<Path>>(
        self,
        path: P,
        options: EvalOptions,
    ) -> Result<V> {
        let buffer = fs::read(path.as_ref())?;
        let file_name = CString::new(
            path.as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )?;

        V::from_js(self, unsafe {
            let val = self.eval_raw(buffer, file_name.as_c_str(), options.to_flag())?;
            Value::from_js_value(self, val)
        })
    }

    /// Compile a module for later use.
    pub fn compile<N, S>(self, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        Module::evaluate(self, name, source)
    }

    /// Returns the global object of this context.
    pub fn globals(self) -> Object<'js> {
        unsafe {
            let v = qjs::JS_GetGlobalObject(self.ctx.as_ptr());
            Object::from_js_value(self, v)
        }
    }

    /// Creates promise and resolving functions.
    pub fn promise(self) -> Result<(Object<'js>, Function<'js>, Function<'js>)> {
        let mut funcs = mem::MaybeUninit::<(qjs::JSValue, qjs::JSValue)>::uninit();

        Ok(unsafe {
            let promise = self.handle_exception(qjs::JS_NewPromiseCapability(
                self.ctx.as_ptr(),
                funcs.as_mut_ptr() as _,
            ))?;
            let (then, catch) = funcs.assume_init();
            (
                Object::from_js_value(self, promise),
                Function::from_js_value(self, then),
                Function::from_js_value(self, catch),
            )
        })
    }

    pub(crate) unsafe fn get_opaque(self) -> &'js mut Opaque {
        let rt = qjs::JS_GetRuntime(self.ctx.as_ptr());
        &mut *(qjs::JS_GetRuntimeOpaque(rt) as *mut _)
    }

    /// Spawn future using configured async runtime
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn<F, T>(&self, future: F)
    where
        F: Future<Output = T> + ParallelSend + 'static,
        T: ParallelSend + 'static,
    {
        let opaque = unsafe { self.get_opaque() };
        opaque.get_spawner().spawn(future);
    }
}

mod test {
    #[cfg(feature = "exports")]
    #[test]
    fn exports() {
        use crate::{intrinsic, Context, Function, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::custom::<(intrinsic::Promise, intrinsic::Eval)>(&runtime).unwrap();
        ctx.with(|ctx| {
            let module = ctx
                .compile("test", "export default async () => 1;")
                .unwrap();
            let func: Function = module.get("default").unwrap();
            func.call::<(), ()>(()).unwrap();
        });
    }

    #[test]
    fn eval() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: String = ctx
                .eval(
                    r#"
                    function test() {
                        var foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                )
                .unwrap();

            assert_eq!("bar".to_string(), res);
        })
    }

    #[test]
    #[should_panic(expected = "'foo' is not defined")]
    fn eval_with_sloppy_code() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let _: String = ctx
                .eval(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                )
                .unwrap();
        })
    }

    #[test]
    fn eval_with_options_no_strict_sloppy_code() {
        use crate::{Context, EvalOptions, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: String = ctx
                .eval_with_options(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                    EvalOptions {
                        strict: false,
                        ..Default::default()
                    },
                )
                .unwrap();

            assert_eq!("bar".to_string(), res);
        })
    }

    #[test]
    #[should_panic(expected = "'foo' is not defined")]
    fn eval_with_options_strict_sloppy_code() {
        use crate::{Context, EvalOptions, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let _: String = ctx
                .eval_with_options(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                    EvalOptions {
                        strict: true,
                        ..Default::default()
                    },
                )
                .unwrap();
        })
    }
}
