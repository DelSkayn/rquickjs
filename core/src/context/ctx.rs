use crate::{
    handle_exception, markers::Invariant, qjs, runtime::Opaque, Context, FromJs, Function, Module,
    Object, Result, Value,
};

#[cfg(feature = "futures")]
use std::future::Future;

#[cfg(feature = "futures")]
use crate::ParallelSend;

#[cfg(feature = "registery")]
use crate::RegisteryKey;

use std::{
    ffi::{CStr, CString},
    fs,
    marker::PhantomData,
    mem,
    path::Path,
};

/// Context in use, passed to [`Context::with`].
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'js> {
    pub(crate) ctx: *mut qjs::JSContext,
    marker: Invariant<'js>,
}

impl<'js> Ctx<'js> {
    pub(crate) fn from_ptr(ctx: *mut qjs::JSContext) -> Self {
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
        let val = qjs::JS_Eval(self.ctx, src.as_ptr(), len as _, file_name.as_ptr(), flag);
        handle_exception(self, val)
    }

    pub(crate) fn eval_with_flag<V: FromJs<'js>, S: Into<Vec<u8>>>(
        self,
        source: S,
        flag: i32,
    ) -> Result<V> {
        let file_name = unsafe { CStr::from_bytes_with_nul_unchecked(b"eval_script\0") };
        V::from_js(self, unsafe {
            let val = self.eval_raw(source, file_name, flag as i32)?;
            Value::from_js_value(self, val)
        })
    }

    pub(crate) fn eval_file_with_flag<V: FromJs<'js>, P: AsRef<Path>>(
        self,
        path: P,
        flag: i32,
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
            let val = self.eval_raw(buffer, file_name.as_c_str(), flag as i32)?;
            Value::from_js_value(self, val)
        })
    }

    /// Evaluate a script in global context.
    /// This enforces strict mode.
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(self, source: S) -> Result<V> {
        self.eval_with_flag(
            source,
            (qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_STRICT) as i32,
        )
    }

    /// Evaluate a script in global context.
    /// This does not enforce strict mode.
    pub fn eval_nonstrict<V: FromJs<'js>, S: Into<Vec<u8>>>(self, source: S) -> Result<V> {
        self.eval_with_flag(source, qjs::JS_EVAL_TYPE_GLOBAL as i32)
    }

    /// Evaluate a script directly from a file.
    /// This enforces strict mode.
    pub fn eval_file<V: FromJs<'js>, P: AsRef<Path>>(self, path: P) -> Result<V> {
        self.eval_file_with_flag(
            path,
            (qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_STRICT) as i32,
        )
    }

    /// Evaluate a script directly from a file.
    /// This does not enforce strict mode.
    pub fn eval_file_nonstrict<V: FromJs<'js>, P: AsRef<Path>>(self, path: P) -> Result<V> {
        self.eval_file_with_flag(path, qjs::JS_EVAL_TYPE_GLOBAL as i32)
    }

    /// Compile a module for later use.
    pub fn compile<N, S>(self, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let module = Module::new(self, name, source)?;
        module.eval()
    }

    /// Returns the global object of this context.
    pub fn globals(self) -> Object<'js> {
        unsafe {
            let v = qjs::JS_GetGlobalObject(self.ctx);
            Object::from_js_value(self, v)
        }
    }

    /// Creates promise and resolving functions.
    pub fn promise(self) -> Result<(Object<'js>, Function<'js>, Function<'js>)> {
        let mut funcs = mem::MaybeUninit::<(qjs::JSValue, qjs::JSValue)>::uninit();

        Ok(unsafe {
            let promise = handle_exception(
                self,
                qjs::JS_NewPromiseCapability(self.ctx, funcs.as_mut_ptr() as _),
            )?;
            let (then, catch) = funcs.assume_init();
            (
                Object::from_js_value(self, promise),
                Function::from_js_value(self, then),
                Function::from_js_value(self, catch),
            )
        })
    }

    pub(crate) unsafe fn get_opaque(self) -> &'js mut Opaque {
        let rt = qjs::JS_GetRuntime(self.ctx);
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

#[cfg(feature = "registery")]
impl<'js> Ctx<'js> {
    /// Store a value in the registery so references to it can be kept outside the scope of context use.
    ///
    /// A registered value can be retrieved from any context belonging to the same runtime.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "registery")))]
    pub fn register(self, v: Value<'js>) -> RegisteryKey {
        unsafe {
            let register = self.get_opaque();
            let key = RegisteryKey(v.into_js_value());
            register.registery.insert(key);
            key
        }
    }

    /// Remove a value from the registery.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "registery")))]
    pub fn deregister(self, k: RegisteryKey) -> Option<Value<'js>> {
        unsafe {
            let register = self.get_opaque();
            if (*register).registery.remove(&k) {
                Some(Value::from_js_value(self, k.0))
            } else {
                None
            }
        }
    }

    /// Get a value from the registery.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "registery")))]
    pub fn get_register(self, k: RegisteryKey) -> Option<Value<'js>> {
        unsafe {
            let opaque = self.get_opaque();
            if opaque.registery.contains(&k) {
                Some(Value::from_js_value_const(self, k.0))
            } else {
                None
            }
        }
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
        use crate::{Context, Error, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: Result<String, Error> = ctx.eval(
                r#"
                    function test() {
                        var foo = "bar";
                        return foo;
                    }

                    test()
                "#,
            );

            assert_eq!("bar".to_string(), res.unwrap());
        })
    }

    #[test]
    fn eval_with_sloppy_code() {
        use crate::{Context, Error, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: Result<String, Error> = ctx.eval(
                r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
            );

            if let Err(Error::Exception {
                message,
                file: _,
                line: _,
                stack: _,
            }) = res
            {
                assert_eq!("'foo' is not defined", message);
            } else {
                panic!("Unexpected result: {:?}", res);
            }
        })
    }

    #[test]
    fn eval_nonstrict() {
        use crate::{Context, Error, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: Result<String, Error> = ctx.eval_nonstrict(
                r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
            );

            assert_eq!("bar".to_string(), res.unwrap());
        })
    }
}
