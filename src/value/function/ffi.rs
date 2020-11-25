use super::StaticFn;
use crate::{
    context::Ctx, qjs, runtime::Opaque, value::handle_panic, ArgsValue, FromJs, FromJsArgs, IntoJs,
    Result, Value,
};
use std::{ffi::CString, marker::PhantomData, panic, panic::AssertUnwindSafe, ptr};

#[repr(transparent)]
pub struct FuncOpaque<'js>(Box<dyn Fn(Ctx<'js>, Value<'js>, ArgsValue<'js>) -> Result<Value<'js>>>);

impl<'js> FuncOpaque<'js> {
    pub fn new<F>(func: F) -> Self
    where
        F: Fn(Ctx<'js>, Value<'js>, ArgsValue<'js>) -> Result<Value<'js>> + 'static,
    {
        Self(Box::new(func))
    }

    unsafe fn call(
        &self,
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> Result<qjs::JSValue> {
        let ctx = Ctx::from_ptr(ctx);

        let this = Value::from_js_value_const(ctx, this)?;
        let args = ArgsValue::from_value_count_const(ctx, argc as usize, argv);

        let res = (self.0)(ctx, this, args)?;

        Ok(res.into_js_value())
    }

    pub unsafe fn to_js_value(self, ctx: Ctx<'_>) -> qjs::JSValue {
        let class_id = ctx.get_opaque().func_class;
        let obj = qjs::JS_NewObjectClass(ctx.ctx, class_id as i32);
        qjs::JS_SetOpaque(obj, Box::into_raw(Box::new(self)) as *mut _);
        obj
    }

    pub unsafe fn new_fn_class(rt: *mut qjs::JSRuntime) -> qjs::JSClassID {
        let mut class_id = 0;
        qjs::JS_NewClassID(&mut class_id);
        let class_def = qjs::JSClassDef {
            class_name: b"RustFunc\0".as_ptr() as *const _,
            finalizer: Some(cb_finalizer),
            gc_mark: None,
            call: Some(cb_call),
            exotic: ptr::null_mut(),
        };
        assert!(qjs::JS_NewClass(rt, class_id, &class_def) == 0);
        class_id
    }
}

pub struct FuncStatic<'js, F>(PhantomData<(&'js (), F)>);

impl<'js, F> FuncStatic<'js, F>
where
    F: StaticFn<'js>,
{
    unsafe fn _call(
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> Result<qjs::JSValue> {
        let ctx = Ctx::from_ptr(ctx);
        let this = Value::from_js_value_const(ctx, this)?;
        let this = F::This::from_js(ctx, this)?;
        let multi = ArgsValue::from_value_count_const(ctx, argc as usize, argv);
        let args = F::Args::from_js_args(ctx, multi)?;
        let res = F::call(ctx, this, args)?;
        let res = res.into_js(ctx)?;
        Ok(res.into_js_value())
    }

    pub unsafe extern "C" fn call(
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> qjs::JSValue {
        //TODO implement some form of poisoning to harden against broken invariants.
        handle_panic(
            ctx,
            AssertUnwindSafe(|| {
                //TODO catch unwind
                Self::_call(ctx, this, argc, argv).unwrap_or_else(|error| {
                    let error = error.to_string();
                    let error_str = CString::new(error).unwrap();
                    qjs::JS_ThrowInternalError(ctx, error_str.as_ptr())
                })
            }),
        )
    }
}

unsafe extern "C" fn cb_call(
    ctx: *mut qjs::JSContext,
    func: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    _flags: qjs::c_int,
) -> qjs::JSValue {
    let ctx = Ctx::from_ptr(ctx);
    let fn_class = ctx.get_opaque().func_class;
    let fn_opaque = &*(qjs::JS_GetOpaque2(ctx.ctx, func, fn_class) as *mut FuncOpaque);
    handle_panic(
        ctx.ctx,
        AssertUnwindSafe(|| {
            fn_opaque
                .call(ctx.ctx, this, argc, argv)
                .unwrap_or_else(|error| error.throw(ctx))
        }),
    )
}

unsafe extern "C" fn cb_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let rt_opaque = &*(qjs::JS_GetRuntimeOpaque(rt) as *mut Opaque);
    let _fn_opaque =
        Box::<FuncOpaque>::from_raw(qjs::JS_GetOpaque(val, rt_opaque.func_class) as *mut _);
}
