use crate::{
    markers::Invariant,
    qjs,
    runtime::Opaque,
    value::{
        self,
        rf::{JsObjectRef, JsStringRef},
        String,
    },
    Context, FromJs, Function, Object, RegisteryKey, Result, Value,
};

use crate::Module;

use std::{
    ffi::{CStr, CString},
    fs,
    marker::PhantomData,
    mem,
    path::Path,
};

/// Context in use, passed to [`Context::with`](struct.Context.html#method.with).
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

    unsafe fn _eval<S: Into<Vec<u8>>>(
        self,
        source: S,
        file_name: &CStr,
        flag: i32,
    ) -> Result<qjs::JSValue> {
        let src = source.into();
        let len = src.len();
        let src = CString::new(src)?;
        let val = qjs::JS_Eval(self.ctx, src.as_ptr(), len as _, file_name.as_ptr(), flag);
        value::handle_exception(self, val)?;
        Ok(val)
    }

    /// Evaluate a script in global context
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(self, source: S) -> Result<V> {
        let file_name = unsafe { CStr::from_bytes_with_nul_unchecked(b"eval_script\0") };
        let flag = qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_STRICT;
        unsafe {
            let val = self._eval(source, file_name, flag as i32)?;
            let val = Value::from_js_value(self, val)?;
            V::from_js(self, val)
        }
    }

    /// Evaluate a script directly from a file.
    pub fn eval_file<V: FromJs<'js>, P: AsRef<Path>>(self, path: P) -> Result<V> {
        let buffer = fs::read(path.as_ref())?;
        let file_name = CString::new(
            path.as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )?;
        let flag = qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_STRICT;
        unsafe {
            let val = self._eval(buffer, file_name.as_c_str(), flag as i32)?;
            let val = Value::from_js_value(self, val)?;
            V::from_js(self, val)
        }
    }

    /// Compile a module for later use.
    pub fn compile<Sa, Sb>(self, name: Sa, source: Sb) -> Result<Module<'js>>
    where
        Sa: Into<Vec<u8>>,
        Sb: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;
        unsafe {
            let js_val = self._eval(source, name.as_c_str(), flag as i32)?;
            value::handle_exception(self, js_val)?;
            let ret = qjs::JS_EvalFunction(self.ctx, js_val);
            value::handle_exception(self, ret)?;
            Ok(Module::from_js_value(self, js_val))
        }
    }

    /// Coerce a value to a string in the same way javascript would coerce values.
    pub fn coerce_string(self, v: Value<'js>) -> Result<String<'js>> {
        unsafe {
            let js_val = qjs::JS_ToString(self.ctx, v.as_js_value());
            value::handle_exception(self, js_val)?;
            // js_val should be a string now
            // String itself will check for the tag when debug_assertions are enabled
            // but is should always be string
            Ok(String(JsStringRef::from_js_value(self, js_val)))
        }
    }

    /// Coerce a value to a `i32` in the same way javascript would coerce values.
    pub fn coerce_i32(self, v: Value<'js>) -> Result<i32> {
        unsafe {
            let mut val: i32 = 0;
            if qjs::JS_ToInt32(self.ctx, &mut val, v.as_js_value()) < 0 {
                return Err(value::get_exception(self));
            }
            Ok(val)
        }
    }

    pub fn coerce_i64(self, v: Value<'js>) -> Result<i64> {
        unsafe {
            let mut val: i64 = 0;
            if qjs::JS_ToInt64(self.ctx, &mut val, v.as_js_value()) < 0 {
                return Err(value::get_exception(self));
            }
            Ok(val)
        }
    }

    pub fn coerce_u64(self, v: Value<'js>) -> Result<u64> {
        unsafe {
            let mut val: u64 = 0;
            if qjs::JS_ToIndex(self.ctx, &mut val, v.as_js_value()) < 0 {
                return Err(value::get_exception(self));
            }
            Ok(val)
        }
    }

    pub fn coerce_f64(self, v: Value<'js>) -> Result<f64> {
        unsafe {
            let mut val: f64 = 0.0;
            if qjs::JS_ToFloat64(self.ctx, &mut val, v.as_js_value()) < 0 {
                return Err(value::get_exception(self));
            }
            Ok(val)
        }
    }

    pub fn coerce_bool(self, v: Value<'js>) -> Result<bool> {
        unsafe {
            let val = qjs::JS_ToBool(self.ctx, v.as_js_value());
            if val < 0 {
                return Err(value::get_exception(self));
            }
            Ok(val == 1)
        }
    }

    /// Returns the global object of this context.
    pub fn globals(self) -> Object<'js> {
        unsafe {
            let v = qjs::JS_GetGlobalObject(self.ctx);
            Object(JsObjectRef::from_js_value(self, v))
        }
    }

    /// Creates promise and resolving functions.
    pub fn promise(self) -> Result<(Object<'js>, Function<'js>, Function<'js>)> {
        let mut funcs = mem::MaybeUninit::<(qjs::JSValue, qjs::JSValue)>::uninit();

        Ok(unsafe {
            let promise = Object(JsObjectRef::from_js_value(
                self,
                value::handle_exception(
                    self,
                    qjs::JS_NewPromiseCapability(self.ctx, funcs.as_mut_ptr() as *mut qjs::JSValue),
                )?,
            ));
            let (then, catch) = funcs.assume_init();
            (
                promise,
                Function(JsObjectRef::from_js_value(self, then)),
                Function(JsObjectRef::from_js_value(self, catch)),
            )
        })
    }

    /// Store a value in the registery so references to it can be kept outside the scope of context use.
    ///
    /// A registered value can be retrieved from any context belonging to the same runtime.
    pub fn register(self, v: Value<'js>) -> RegisteryKey {
        unsafe {
            let register = self.get_opaque();
            let key = RegisteryKey(v.into_js_value());
            register.registery.insert(key);
            key
        }
    }

    /// Remove a value from the registery.
    pub fn deregister(self, k: RegisteryKey) -> Option<Value<'js>> {
        unsafe {
            let register = self.get_opaque();
            if (*register).registery.remove(&k) {
                Some(Value::from_js_value(self, k.0).unwrap())
            } else {
                None
            }
        }
    }

    /// Get a value from the registery.
    pub fn get_register(self, k: RegisteryKey) -> Option<Value<'js>> {
        unsafe {
            let register = self.get_opaque();
            if register.registery.contains(&k) {
                let value = Value::from_js_value_const(self, k.0).unwrap();
                Some(value)
            } else {
                None
            }
        }
    }

    pub(crate) unsafe fn get_opaque(self) -> &'js mut Opaque {
        let ptr = qjs::JS_GetRuntimeOpaque(qjs::JS_GetRuntime(self.ctx));
        &mut *(ptr as *mut _)
    }
}

mod test {

    #[cfg(feature = "exports")]
    #[test]
    fn exports() {
        use crate::{Context, FromJs, Function, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::build(&runtime)
            .promises(true)
            .eval(true)
            .build()
            .unwrap();
        ctx.with(|ctx| {
            let module = ctx
                .compile("test", "export default async () => 1;")
                .unwrap();
            for (key, value) in module.export_list() {
                if key.to_string().unwrap() == "default" {
                    let func = Function::from_js(ctx, value).unwrap();
                    func.call::<(), ()>(()).unwrap();
                }
            }
        });
    }
}
