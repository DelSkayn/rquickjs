use std::{
    ffi::{CStr, CString},
    fs, mem,
    path::Path,
    ptr::NonNull,
};

#[cfg(feature = "futures")]
use std::future::Future;

#[cfg(feature = "futures")]
use crate::AsyncContext;
use crate::{
    markers::Invariant, qjs, runtime::raw::Opaque, Context, Error, FromJs, Function, IntoJs,
    Module, Object, Result, String, Value,
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
    _marker: Invariant<'js>,
}

unsafe impl Send for Ctx<'_> {}

impl<'js> Ctx<'js> {
    /// Create a new `Ctx` from a pointer to the context and a invariant lifetime.
    ///
    /// # Safety
    /// User must ensure that a lock was acquired over the runtime and that invariant is a unique
    /// lifetime which can't be coerced to a lifetime outside the scope of the lock of to the
    /// lifetime of another runtime.
    pub unsafe fn from_ptr_invariant(ctx: NonNull<qjs::JSContext>, inv: Invariant<'js>) -> Self {
        Ctx { ctx, _marker: inv }
    }

    pub(crate) fn as_ptr(&self) -> *mut qjs::JSContext {
        self.ctx.as_ptr()
    }

    pub(crate) unsafe fn from_ptr(ctx: *mut qjs::JSContext) -> Self {
        let ctx = NonNull::new_unchecked(ctx);
        Ctx {
            ctx,
            _marker: Invariant::new(),
        }
    }

    pub(crate) unsafe fn new(ctx: &'js Context) -> Self {
        Ctx {
            ctx: ctx.ctx,
            _marker: Invariant::new(),
        }
    }

    #[cfg(feature = "futures")]
    pub(crate) unsafe fn new_async(ctx: &'js AsyncContext) -> Self {
        Ctx {
            ctx: ctx.ctx,
            _marker: Invariant::new(),
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

    /// Returns the last raised javascript exception, if there is no exception the javascript value `null` is returned.
    ///
    /// # Usage
    /// ```
    /// # use rquickjs::{Error, Context, Runtime};
    /// # let rt = Runtime::new().unwrap();
    /// # let ctx = Context::full(&rt).unwrap();
    /// # ctx.with(|ctx|{
    /// if let Err(Error::Exception) = ctx.eval::<(),_>("throw 3"){
    ///     assert_eq!(ctx.catch().as_int(),Some(3));
    /// # }else{
    /// #    panic!()
    /// }
    /// # });
    /// ```
    pub fn catch(self) -> Value<'js> {
        unsafe {
            let v = qjs::JS_GetException(self.ctx.as_ptr());
            Value::from_js_value(self, v)
        }
    }

    /// Throws a javascript value as a new exception.
    /// Always returns `Error::Exception`;
    pub fn throw(self, value: Value<'js>) -> Error {
        unsafe {
            let v = value.into_js_value();
            qjs::JS_Throw(self.ctx.as_ptr(), v);
        }
        Error::Exception
    }

    /// Parse json into a javascript value.
    pub fn json_parse<S>(self, json: S) -> Result<Value<'js>>
    where
        S: Into<Vec<u8>>,
    {
        self.json_parse_ext(json, false)
    }

    /// Parse json into a javascript value, possibly allowing extended syntax support.
    ///
    /// If allow_extensions is true, this function will allow extended json syntex.
    /// Extended syntax allows comments, single quoted strings, non string property names, trailing
    /// comma's and hex, oct and binary numbers.
    pub fn json_parse_ext<S>(self, json: S, allow_extensions: bool) -> Result<Value<'js>>
    where
        S: Into<Vec<u8>>,
    {
        let src = json.into();
        let len = src.len();
        let mut src = CString::new(src)?;
        unsafe {
            let flag = if allow_extensions {
                qjs::JS_PARSE_JSON_EXT as i32
            } else {
                0i32
            };
            let name = b"<input>\0";
            let v = qjs::JS_ParseJSON2(
                self.as_ptr(),
                src.as_ptr().cast(),
                len,
                name.as_ptr().cast(),
                flag,
            );
            self.handle_exception(v)?;
            Ok(Value::from_js_value(self, v))
        }
    }

    /// Stringify a javascript value into its JSON representation
    pub fn json_stringify<V>(self, value: V) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
    {
        self.json_stringify_inner(
            &value.into_js(self)?,
            &Value::new_undefined(self),
            &Value::new_undefined(self),
        )
    }

    /// Stringify a javascript value into its JSON representation with a possible replacer.
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    pub fn json_stringify_replacer<V, R>(self, value: V, replacer: R) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
    {
        self.json_stringify_inner(
            &value.into_js(self)?,
            &replacer.into_js(self)?,
            &Value::new_undefined(self),
        )
    }

    /// Stringify a javascript value into its JSON representation with a possible replacer and
    /// spaces
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    ///
    /// Space is either a number or a string which is used to insert whitespace into the output
    /// string for readability purposes. This behaves the same as the space argument for
    /// `JSON.stringify`.
    pub fn json_stringify_replacer_space<V, R, S>(
        self,
        value: V,
        replacer: R,
        space: S,
    ) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
        S: IntoJs<'js>,
    {
        self.json_stringify_inner(
            &value.into_js(self)?,
            &replacer.into_js(self)?,
            &space.into_js(self)?,
        )
    }

    // Inner non-generic version of json stringify>
    fn json_stringify_inner(
        self,
        value: &Value<'js>,
        replacer: &Value<'js>,
        space: &Value<'js>,
    ) -> Result<Option<String<'js>>> {
        unsafe {
            let res = qjs::JS_JSONStringify(
                self.as_ptr(),
                value.as_js_value(),
                replacer.as_js_value(),
                space.as_js_value(),
            );
            self.handle_exception(res)?;
            let v = Value::from_js_value(self, res);
            if v.is_undefined() {
                Ok(None)
            } else {
                let v = v.into_string().expect(
                    "JS_JSONStringify did not return either an exception, undefined, or a string",
                );
                Ok(Some(v))
            }
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

    pub(crate) unsafe fn get_opaque(self) -> &'js mut Opaque<'js> {
        let rt = qjs::JS_GetRuntime(self.ctx.as_ptr());
        &mut *(qjs::JS_GetRuntimeOpaque(rt) as *mut _)
    }

    /// Spawn future using configured async runtime
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'js,
    {
        unsafe { self.get_opaque().spawner().push(future) }
    }
}

#[cfg(test)]
mod test {

    #[cfg(feature = "exports")]
    #[test]
    fn exports() {
        use crate::{context::intrinsic, Context, Function, Runtime};

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
        use crate::{CatchResultExt, Context, Runtime};

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
                .catch(ctx)
                .unwrap();
        })
    }

    #[test]
    fn eval_with_options_no_strict_sloppy_code() {
        use crate::{context::EvalOptions, Context, Runtime};

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
        use crate::{context::EvalOptions, CatchResultExt, Context, Runtime};

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
                .catch(ctx)
                .unwrap();
        })
    }
}
