use crate::{
    handle_exception, markers::Invariant, qjs, runtime::Opaque, BeforeInit, Context, FromJs,
    Function, Module, Object, Result, Value,
};

#[cfg(feature = "registery")]
use crate::RegisteryKey;

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
        handle_exception(self, val)
    }

    /// Evaluate a script in global context
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(self, source: S) -> Result<V> {
        let file_name = unsafe { CStr::from_bytes_with_nul_unchecked(b"eval_script\0") };
        let flag = qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_STRICT;
        V::from_js(self, unsafe {
            let val = self._eval(source, file_name, flag as i32)?;
            Value::from_js_value(self, val)
        })
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
        V::from_js(self, unsafe {
            let val = self._eval(buffer, file_name.as_c_str(), flag as i32)?;
            Value::from_js_value(self, val)
        })
    }

    /// Compile a module for later use.
    pub fn compile<N, S>(self, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;
        unsafe {
            let js_val = self._eval(source, name.as_c_str(), flag as i32)?;
            //handle_exception(self, js_val)?;
            let ret = qjs::JS_EvalFunction(self.ctx, js_val);
            handle_exception(self, ret)?;
            Ok(Module::from_js_value(self, js_val))
        }
    }

    pub fn compile_only<N, S>(self, name: N, source: S) -> Result<Module<'js, BeforeInit>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;
        unsafe {
            let js_val = self._eval(source, name.as_c_str(), flag as i32)?;
            handle_exception(self, js_val)?;
            Ok(Module::<BeforeInit>::from_js_value(self, js_val))
        }
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
        let ptr = qjs::JS_GetRuntimeOpaque(qjs::JS_GetRuntime(self.ctx));
        &mut *(ptr as *mut _)
    }
}

#[cfg(feature = "registery")]
impl<'js> Ctx<'js> {
    /// Store a value in the registery so references to it can be kept outside the scope of context use.
    ///
    /// A registered value can be retrieved from any context belonging to the same runtime.
    ///
    /// # Features
    /// This method is only available if a `registery` feature is enabled.
    pub fn register(self, v: Value<'js>) -> RegisteryKey {
        unsafe {
            let register = self.get_opaque();
            let key = RegisteryKey(v.into_js_value());
            register.registery.insert(key);
            key
        }
    }

    /// Remove a value from the registery.
    ///
    /// # Features
    /// This method is only available if a `registery` feature is enabled.
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
    ///
    /// # Features
    /// This method is only available if a `registery` feature is enabled.
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
        use crate::{Context, Function, Runtime};

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
            let func: Function = module.get("default").unwrap();
            func.call::<(), ()>(()).unwrap();
        });
    }
}
