use super::Input;
use crate::{handle_panic, qjs, Ctx, Result, Value};
use std::{ops::Deref, panic, panic::AssertUnwindSafe, ptr};

static mut FUNC_CLASS: qjs::JSClassID = 0;

type BoxedFunc<'js> = Box<dyn Fn(&Input<'js>) -> Result<Value<'js>>>;

#[repr(transparent)]
pub struct JsFunction<'js>(BoxedFunc<'js>);

impl<'js> Deref for JsFunction<'js> {
    type Target = BoxedFunc<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> JsFunction<'js> {
    pub fn new<F>(func: F) -> Self
    where
        F: Fn(&Input<'js>) -> Result<Value<'js>> + 'static,
    {
        Self(Box::new(func))
    }

    pub unsafe fn into_js_value(self, ctx: Ctx<'_>) -> qjs::JSValue {
        let obj = qjs::JS_NewObjectClass(ctx.ctx, FUNC_CLASS as _);
        qjs::JS_SetOpaque(obj, Box::into_raw(Box::new(self)) as _);
        obj
    }

    unsafe fn _call(
        &self,
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> Result<qjs::JSValue> {
        let input = Input::new_raw(ctx, this, argc, argv);

        let res = self.0(&input)?;

        Ok(res.into_js_value())
    }

    pub unsafe fn register(rt: *mut qjs::JSRuntime) {
        qjs::JS_NewClassID(&mut FUNC_CLASS);
        if 0 == qjs::JS_IsRegisteredClass(rt, FUNC_CLASS) {
            let class_def = qjs::JSClassDef {
                class_name: b"RustFunction\0".as_ptr() as *const _,
                finalizer: Some(Self::finalizer),
                gc_mark: None,
                call: Some(Self::call),
                exotic: ptr::null_mut(),
            };
            assert!(qjs::JS_NewClass(rt, FUNC_CLASS, &class_def) == 0);
        }
    }

    unsafe extern "C" fn call(
        ctx: *mut qjs::JSContext,
        func: qjs::JSValue,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
        _flags: qjs::c_int,
    ) -> qjs::JSValue {
        let ctx = Ctx::from_ptr(ctx);
        let opaque = &*(qjs::JS_GetOpaque2(ctx.ctx, func, FUNC_CLASS) as *mut Self);

        handle_panic(
            ctx.ctx,
            AssertUnwindSafe(|| {
                opaque
                    ._call(ctx.ctx, this, argc, argv)
                    .unwrap_or_else(|error| error.throw(ctx))
            }),
        )
    }

    unsafe extern "C" fn finalizer(_rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let _opaque = Box::from_raw(qjs::JS_GetOpaque(val, FUNC_CLASS) as *mut Self);
    }
}
