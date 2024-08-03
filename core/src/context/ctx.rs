#[cfg(feature = "futures")]
use std::future::Future;
use std::{
    ffi::{CStr, CString},
    fs,
    mem::{self, MaybeUninit},
    path::Path,
    ptr::NonNull,
    result::Result as StdResult,
};

#[cfg(feature = "futures")]
use crate::AsyncContext;
use crate::{
    cstr,
    markers::Invariant,
    qjs,
    runtime::{opaque::Opaque, UserData, UserDataError, UserDataGuard, UserDataMap},
    Atom, Error, FromJs, Function, IntoJs, Object, Promise, Result, String, Value,
};

use super::Context;

/// Eval options.
#[non_exhaustive]
pub struct EvalOptions {
    /// Global code.
    pub global: bool,
    /// Force 'strict' mode.
    pub strict: bool,
    /// Don't include the stack frames before this eval in the Error() backtraces.
    pub backtrace_barrier: bool,
    /// Support top-level-await.
    pub promise: bool,
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

        if self.promise {
            flag |= qjs::JS_EVAL_FLAG_ASYNC;
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
            promise: false,
        }
    }
}

/// Context in use, passed to [`Context::with`].
#[derive(Debug)]
pub struct Ctx<'js> {
    ctx: NonNull<qjs::JSContext>,
    _marker: Invariant<'js>,
}

impl<'js> Clone for Ctx<'js> {
    fn clone(&self) -> Self {
        unsafe { qjs::JS_DupContext(self.ctx.as_ptr()) };
        Ctx {
            ctx: self.ctx,
            _marker: self._marker,
        }
    }
}

impl<'js> Drop for Ctx<'js> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) };
    }
}

unsafe impl Send for Ctx<'_> {}

impl<'js> Ctx<'js> {
    pub(crate) fn as_ptr(&self) -> *mut qjs::JSContext {
        self.ctx.as_ptr()
    }

    pub(crate) unsafe fn from_ptr(ctx: *mut qjs::JSContext) -> Self {
        unsafe { qjs::JS_DupContext(ctx) };
        let ctx = NonNull::new_unchecked(ctx);
        Ctx {
            ctx,
            _marker: Invariant::new(),
        }
    }

    pub(crate) unsafe fn new(ctx: &'js Context) -> Self {
        unsafe { qjs::JS_DupContext(ctx.0.ctx.as_ptr()) };
        Ctx {
            ctx: ctx.0.ctx,
            _marker: Invariant::new(),
        }
    }

    #[cfg(feature = "futures")]
    pub(crate) unsafe fn new_async(ctx: &'js AsyncContext) -> Self {
        unsafe { qjs::JS_DupContext(ctx.0.ctx.as_ptr()) };
        Ctx {
            ctx: ctx.0.ctx,
            _marker: Invariant::new(),
        }
    }

    pub(crate) unsafe fn eval_raw<S: Into<Vec<u8>>>(
        &self,
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
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(&self, source: S) -> Result<V> {
        self.eval_with_options(source, Default::default())
    }

    /// Evaluate a script in global context with top level await support.
    ///
    /// This function always returns a promise which resolves to the result of the evaluated
    /// expression.
    pub fn eval_promise<S: Into<Vec<u8>>>(&self, source: S) -> Result<Promise<'js>> {
        self.eval_with_options(
            source,
            EvalOptions {
                promise: true,
                ..Default::default()
            },
        )
    }

    /// Evaluate a script with the given options.
    pub fn eval_with_options<V: FromJs<'js>, S: Into<Vec<u8>>>(
        &self,
        source: S,
        options: EvalOptions,
    ) -> Result<V> {
        let file_name = cstr!("eval_script");

        V::from_js(self, unsafe {
            let val = self.eval_raw(source, file_name, options.to_flag())?;
            Value::from_js_value(self.clone(), val)
        })
    }

    /// Evaluate a script directly from a file.
    pub fn eval_file<V: FromJs<'js>, P: AsRef<Path>>(&self, path: P) -> Result<V> {
        self.eval_file_with_options(path, Default::default())
    }

    pub fn eval_file_with_options<V: FromJs<'js>, P: AsRef<Path>>(
        &self,
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
            Value::from_js_value(self.clone(), val)
        })
    }

    /// Returns the global object of this context.
    pub fn globals(&self) -> Object<'js> {
        unsafe {
            let v = qjs::JS_GetGlobalObject(self.ctx.as_ptr());
            Object::from_js_value(self.clone(), v)
        }
    }

    /// Returns the last raised JavaScript exception, if there is no exception the JavaScript value `null` is returned.
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
    pub fn catch(&self) -> Value<'js> {
        unsafe {
            let v = qjs::JS_GetException(self.ctx.as_ptr());
            Value::from_js_value(self.clone(), v)
        }
    }

    /// Throws a JavaScript value as a new exception.
    /// Always returns `Error::Exception`;
    pub fn throw(&self, value: Value<'js>) -> Error {
        unsafe {
            let v = value.into_js_value();
            qjs::JS_Throw(self.ctx.as_ptr(), v);
        }
        Error::Exception
    }

    /// Parse json into a JavaScript value.
    pub fn json_parse<S>(&self, json: S) -> Result<Value<'js>>
    where
        S: Into<Vec<u8>>,
    {
        self.json_parse_ext(json, false)
    }

    /// Parse json into a JavaScript value, possibly allowing extended syntax support.
    ///
    /// If `allow_extensions` is `true`, this function will allow extended json syntax.
    /// Extended syntax allows comments, single quoted strings, non string property names, trailing
    /// comma's and hex, oct and binary numbers.
    pub fn json_parse_ext<S>(&self, json: S, allow_extensions: bool) -> Result<Value<'js>>
    where
        S: Into<Vec<u8>>,
    {
        let src = json.into();
        let len = src.len();
        let src = CString::new(src)?;
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
                len.try_into().expect(qjs::SIZE_T_ERROR),
                name.as_ptr().cast(),
                flag,
            );
            self.handle_exception(v)?;
            Ok(Value::from_js_value(self.clone(), v))
        }
    }

    /// Stringify a JavaScript value into its JSON representation
    pub fn json_stringify<V>(&self, value: V) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
    {
        self.json_stringify_inner(&value.into_js(self)?, qjs::JS_UNDEFINED, qjs::JS_UNDEFINED)
    }

    /// Stringify a JavaScript value into its JSON representation with a possible replacer.
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    pub fn json_stringify_replacer<V, R>(
        &self,
        value: V,
        replacer: R,
    ) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
    {
        let replacer = replacer.into_js(self)?;

        self.json_stringify_inner(
            &value.into_js(self)?,
            replacer.as_js_value(),
            qjs::JS_UNDEFINED,
        )
    }

    /// Stringify a JavaScript value into its JSON representation with a possible replacer and
    /// spaces
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    ///
    /// Space is either a number or a string which is used to insert whitespace into the output
    /// string for readability purposes. This behaves the same as the space argument for
    /// `JSON.stringify`.
    pub fn json_stringify_replacer_space<V, R, S>(
        &self,
        value: V,
        replacer: R,
        space: S,
    ) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
        S: IntoJs<'js>,
    {
        let replacer = replacer.into_js(self)?;
        let space = space.into_js(self)?;

        self.json_stringify_inner(
            &value.into_js(self)?,
            replacer.as_js_value(),
            space.as_js_value(),
        )
    }

    // Inner non-generic version of json stringify>
    fn json_stringify_inner(
        &self,
        value: &Value<'js>,
        replacer: qjs::JSValueConst,
        space: qjs::JSValueConst,
    ) -> Result<Option<String<'js>>> {
        unsafe {
            let res = qjs::JS_JSONStringify(self.as_ptr(), value.as_js_value(), replacer, space);
            self.handle_exception(res)?;
            let v = Value::from_js_value(self.clone(), res);
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

    /// Creates javascipt promise along with its reject and resolve functions.
    pub fn promise(&self) -> Result<(Promise<'js>, Function<'js>, Function<'js>)> {
        let mut funcs = mem::MaybeUninit::<(qjs::JSValue, qjs::JSValue)>::uninit();

        Ok(unsafe {
            let promise = self.handle_exception(qjs::JS_NewPromiseCapability(
                self.ctx.as_ptr(),
                funcs.as_mut_ptr() as _,
            ))?;
            let (resolve, reject) = funcs.assume_init();
            (
                Promise::from_js_value(self.clone(), promise),
                Function::from_js_value(self.clone(), resolve),
                Function::from_js_value(self.clone(), reject),
            )
        })
    }

    /// Executes a quickjs job.
    ///
    /// Returns wether a job was actually executed.
    /// If this function returned false, no job was pending.
    pub fn execute_pending_job(&self) -> bool {
        let mut ptr = MaybeUninit::<*mut qjs::JSContext>::uninit();
        let rt = unsafe { qjs::JS_GetRuntime(self.ctx.as_ptr()) };
        let res = unsafe { qjs::JS_ExecutePendingJob(rt, ptr.as_mut_ptr()) };
        res != 0
    }

    pub(crate) unsafe fn get_opaque(&self) -> &Opaque<'js> {
        let rt = qjs::JS_GetRuntime(self.ctx.as_ptr());
        &(*qjs::JS_GetRuntimeOpaque(rt).cast::<Opaque>())
    }

    /// Spawn future using configured async runtime
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'js,
    {
        unsafe { self.get_opaque().push(future) }
    }

    /// Create a new `Ctx` from a pointer to the context and a invariant lifetime.
    ///
    /// # Safety
    /// User must ensure that a lock was acquired over the runtime and that invariant is a unique
    /// lifetime which can't be coerced to a lifetime outside the scope of the lock of to the
    /// lifetime of another runtime.
    pub unsafe fn from_raw_invariant(ctx: NonNull<qjs::JSContext>, inv: Invariant<'js>) -> Self {
        unsafe { qjs::JS_DupContext(ctx.as_ptr()) };
        Ctx { ctx, _marker: inv }
    }

    /// Create a new `Ctx` from a pointer to the context and a invariant lifetime.
    ///
    /// # Safety
    /// User must ensure that a lock was acquired over the runtime and that invariant is a unique
    /// lifetime which can't be coerced to a lifetime outside the scope of the lock of to the
    /// lifetime of another runtime.
    pub unsafe fn from_raw(ctx: NonNull<qjs::JSContext>) -> Self {
        unsafe { qjs::JS_DupContext(ctx.as_ptr()) };
        Ctx {
            ctx,
            _marker: Invariant::new(),
        }
    }

    /// Returns the name of the current module or script that is running.
    ///
    /// It called from a javascript callback it will return the current running javascript script
    /// name.
    /// Otherwise it will return none.
    pub fn script_or_module_name(&self, stack_level: isize) -> Option<Atom<'js>> {
        let stack_level = std::os::raw::c_int::try_from(stack_level).unwrap();
        let atom = unsafe { qjs::JS_GetScriptOrModuleName(self.as_ptr(), stack_level) };
        if qjs::__JS_ATOM_NULL as u32 == atom {
            unsafe { qjs::JS_FreeAtom(self.as_ptr(), atom) };
            return None;
        }
        unsafe { Some(Atom::from_atom_val(self.clone(), atom)) }
    }

    /// Runs the quickjs garbage collector for a cycle.
    ///
    /// Quickjs uses reference counting with a collection cycle for cyclic references.
    /// This runs the cyclic reference collector cycle, types which are not part of a reference cycle
    /// will be freed the momement their reference count becomes zero.
    pub fn run_gc(&self) {
        unsafe { qjs::JS_RunGC(qjs::JS_GetRuntime(self.ctx.as_ptr())) }
    }

    /// Store a type in the runtime which can be retrieved later with `Ctx::userdata`.
    ///
    /// Returns the value from the argument if the userdata is currently being accessed and
    /// insertion is not possible.
    /// Otherwise returns the exising value for this type if it existed.
    pub fn store_userdata<U: UserData<'js>>(
        &self,
        data: U,
    ) -> StdResult<Option<Box<U>>, UserDataError<U>> {
        unsafe { self.get_opaque().insert_userdata(data) }
    }

    /// Store multiple types in the runtime which can be retrieved later with `Ctx::userdata`.
    ///
    /// Returns an error if the userdata is currently being accessed.
    pub fn extend_userdata(&self, data: UserDataMap) -> StdResult<(), UserDataError<()>> {
        unsafe { self.get_opaque().extend_userdata(data) }
    }

    /// Remove the userdata of the given type from the userdata storage.
    ///
    /// Returns Err(()) if the userdata is currently being accessed and removing isn't possible.
    /// Returns Ok(None) if userdata of the given type wasn't inserted.
    pub fn remove_userdata<U: UserData<'js>>(
        &self,
    ) -> StdResult<Option<Box<U>>, UserDataError<()>> {
        unsafe { self.get_opaque().remove_userdata() }
    }

    /// Retrieves a borrow to the userdata of the given type from the userdata storage.
    ///
    /// Returns None if userdata of the given type wasn't inserted.
    pub fn userdata<U: UserData<'js>>(&self) -> Option<UserDataGuard<U>> {
        unsafe { self.get_opaque().get_userdata() }
    }

    /// Returns the pointer to the C library context.
    pub fn as_raw(&self) -> NonNull<qjs::JSContext> {
        self.ctx
    }
}

#[cfg(test)]
mod test {
    use crate::CatchResultExt;

    #[test]
    fn exports() {
        use crate::{context::intrinsic, Context, Function, Module, Promise, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::custom::<(intrinsic::Promise, intrinsic::Eval)>(&runtime).unwrap();
        ctx.with(|ctx| {
            let (module, promise) = Module::declare(ctx, "test", "export default async () => 1;")
                .unwrap()
                .eval()
                .unwrap();
            promise.finish::<()>().unwrap();
            let func: Function = module.get("default").unwrap();
            func.call::<(), Promise>(()).unwrap();
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
    fn eval_minimal_test() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: i32 = ctx.eval(" 1 + 1 ").unwrap();
            assert_eq!(2, res);
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
                .catch(&ctx)
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
                .catch(&ctx)
                .unwrap();
        })
    }

    #[test]
    fn json_parse() {
        use crate::{Array, Context, Object, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let v = ctx
                .json_parse(r#"{ "a": { "b": 1, "c": true }, "d": [0,"foo"] }"#)
                .unwrap();
            let obj = v.into_object().unwrap();
            let inner_obj: Object = obj.get("a").unwrap();
            assert_eq!(inner_obj.get::<_, i32>("b").unwrap(), 1);
            assert!(inner_obj.get::<_, bool>("c").unwrap());
            let inner_array: Array = obj.get("d").unwrap();
            assert_eq!(inner_array.get::<i32>(0).unwrap(), 0);
            assert_eq!(inner_array.get::<String>(1).unwrap(), "foo".to_string());
        })
    }

    #[test]
    fn json_parse_extension() {
        use crate::{Array, Context, Object, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let v = ctx
                .json_parse_ext(
                    r#"{ a: { "b": 0xf, "c": 0b11 }, "d": [0o17,'foo'], }"#,
                    true,
                )
                .unwrap();
            let obj = v.into_object().unwrap();
            let inner_obj: Object = obj.get("a").unwrap();
            assert_eq!(inner_obj.get::<_, i32>("b").unwrap(), 0xf);
            assert_eq!(inner_obj.get::<_, i32>("c").unwrap(), 0b11);
            let inner_array: Array = obj.get("d").unwrap();
            assert_eq!(inner_array.get::<i32>(0).unwrap(), 0o17);
            assert_eq!(inner_array.get::<String>(1).unwrap(), "foo".to_string());
        })
    }

    #[test]
    fn json_stringify() {
        use crate::{Array, Context, Object, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let obj_inner = Object::new(ctx.clone()).unwrap();
            obj_inner.set("b", 1).unwrap();
            obj_inner.set("c", true).unwrap();

            let array_inner = Array::new(ctx.clone()).unwrap();
            array_inner.set(0, 0).unwrap();
            array_inner.set(1, "foo").unwrap();

            let obj = Object::new(ctx.clone()).unwrap();
            obj.set("a", obj_inner).unwrap();
            obj.set("d", array_inner).unwrap();

            let str = ctx
                .json_stringify(obj)
                .unwrap()
                .unwrap()
                .to_string()
                .unwrap();

            assert_eq!(str, r#"{"a":{"b":1,"c":true},"d":[0,"foo"]}"#);
        })
    }

    #[test]
    fn userdata() {
        use crate::{runtime::UserData, Context, Function, Runtime};

        pub struct MyUserData<'js> {
            base: Function<'js>,
        }

        unsafe impl<'js> UserData<'js> for MyUserData<'js> {
            type Static = MyUserData<'static>;
        }

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        ctx.with(|ctx| {
            let func = ctx.eval("() => 42").catch(&ctx).unwrap();
            ctx.store_userdata(MyUserData { base: func }).unwrap();
        });

        ctx.with(|ctx| {
            let userdata = ctx.userdata::<MyUserData>().unwrap();

            assert!(ctx.remove_userdata::<MyUserData>().is_err());

            let r: usize = userdata.base.call(()).unwrap();
            assert_eq!(r, 42)
        });

        ctx.with(|ctx| {
            ctx.remove_userdata::<MyUserData>().unwrap().unwrap();
        })
    }
}
