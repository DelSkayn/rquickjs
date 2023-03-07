use super::Input;
use crate::{handle_panic, qjs, ClassId, Ctx, Result, Value};
use std::{ops::Deref, panic::AssertUnwindSafe, ptr};

static FUNC_CLASS_ID: ClassId = ClassId::new();

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

    pub fn class_id() -> qjs::JSClassID {
        FUNC_CLASS_ID.get() as _
    }

    pub unsafe fn into_js_value(self, ctx: Ctx<'_>) -> qjs::JSValue {
        let obj = qjs::JS_NewObjectClass(ctx.ctx, Self::class_id() as _);
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
        let class_id = Self::class_id();
        if 0 == qjs::JS_IsRegisteredClass(rt, class_id) {
            let class_def = qjs::JSClassDef {
                class_name: b"RustFunction\0".as_ptr() as *const _,
                finalizer: Some(Self::finalizer),
                gc_mark: None,
                call: Some(Self::call),
                exotic: ptr::null_mut(),
            };
            assert!(qjs::JS_NewClass(rt, class_id, &class_def) == 0);
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
        let opaque = &*(qjs::JS_GetOpaque2(ctx.ctx, func, Self::class_id()) as *mut Self);

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
        let _opaque = Box::from_raw(qjs::JS_GetOpaque(val, Self::class_id()) as *mut Self);
    }
}
