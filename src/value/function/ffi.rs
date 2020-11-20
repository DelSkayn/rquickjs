use super::StaticFn;
use crate::{
    context::Ctx, qjs, runtime::Opaque, value::handle_panic, ArgsValue, FromJs, FromJsArgs, IntoJs,
    Result, Value,
};
use std::{cell::RefCell, ffi::CString, mem, panic::AssertUnwindSafe};

pub struct FuncOpaque {
    func: Box<
        dyn Fn(
            *mut qjs::JSContext,
            qjs::JSValue,
            std::os::raw::c_int,
            *mut qjs::JSValue,
        ) -> qjs::JSValue,
    >,
}

macro_rules! try_ffi {
    ($ctx:expr,$e:expr) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                let error = format!("{}", e);
                let error_str = CString::new(error).unwrap();
                return qjs::JS_ThrowInternalError($ctx, error_str.as_ptr());
            }
        }
    };
}

pub unsafe extern "C" fn call_fn_static<'js, F>(
    ctx: *mut qjs::JSContext,
    this: qjs::JSValue,
    argc: std::os::raw::c_int,
    argv: *mut qjs::JSValue,
) -> qjs::JSValue
where
    F: StaticFn<'js>,
{
    //TODO implement some form of poisoning to harden against broken invariants.
    handle_panic(
        ctx,
        AssertUnwindSafe(|| {
            //TODO catch unwind
            let ctx = Ctx::from_ptr(ctx);
            let this = try_ffi!(ctx.ctx, Value::from_js_value_const(ctx, this));
            let this = try_ffi!(ctx.ctx, F::This::from_js(ctx, this));
            let multi = ArgsValue::from_value_count_const(ctx, argc as usize, argv);
            let args = try_ffi!(ctx.ctx, F::Args::from_js_args(ctx, multi));
            let value = try_ffi!(ctx.ctx, F::call(ctx, this, args));
            let value = try_ffi!(ctx.ctx, value.into_js(ctx));
            value.into_js_value()
        }),
    )
}

pub unsafe extern "C" fn cb_call(
    ctx: *mut qjs::JSContext,
    func_obj: qjs::JSValue,
    this_val: qjs::JSValue,
    argc: ::std::os::raw::c_int,
    argv: *mut qjs::JSValue,
    _flags: ::std::os::raw::c_int,
) -> qjs::JSValue {
    let c = Ctx::from_ptr(ctx);
    let fn_opaque = qjs::JS_GetOpaque2(ctx, func_obj, c.get_opaque().func_class) as *mut FuncOpaque;
    (&mut (*fn_opaque).func)(ctx, this_val, argc, argv)
}

pub fn wrap_cb_mut<'js, A, T, R, F>(func: F) -> FuncOpaque
where
    A: FromJsArgs<'js>,
    T: FromJs<'js>,
    R: IntoJs<'js>,
    F: FnMut(Ctx<'js>, T, A) -> Result<R> + 'static,
{
    let func = RefCell::new(func);

    FuncOpaque {
        func: Box::new(move |ctx, this, argc, argv| {
            let func = &func;
            handle_panic(
                ctx,
                AssertUnwindSafe(move || unsafe {
                    let ctx = Ctx::from_ptr(ctx);
                    let this = try_ffi!(ctx.ctx, Value::from_js_value_const(ctx, this));
                    let this = try_ffi!(ctx.ctx, T::from_js(ctx, this));
                    let multi = ArgsValue::from_value_count_const(ctx, argc as usize, argv);
                    let args = try_ffi!(ctx.ctx, A::from_js_args(ctx, multi));
                    let value = {
                        let mut func = func.try_borrow_mut()
                        .expect("Mutable function callback is already in use! Could it have been called recursively?");
                        try_ffi!(ctx.ctx, func(ctx, this, args))
                    };
                    let value = try_ffi!(ctx.ctx, value.into_js(ctx));
                    value.into_js_value()
                }),
            )
        }),
    }
}

pub fn wrap_cb<'js, A, T, R, F>(func: F) -> FuncOpaque
where
    A: FromJsArgs<'js>,
    T: FromJs<'js>,
    R: IntoJs<'js>,
    F: Fn(Ctx<'js>, T, A) -> Result<R> + 'static,
{
    FuncOpaque {
        func: Box::new(move |ctx, this, argc, argv| {
            let func = &func;
            handle_panic(
                ctx,
                AssertUnwindSafe(move || unsafe {
                    let ctx = Ctx::from_ptr(ctx);
                    let this = try_ffi!(ctx.ctx, Value::from_js_value_const(ctx, this));
                    let this = try_ffi!(ctx.ctx, T::from_js(ctx, this));
                    let multi = ArgsValue::from_value_count_const(ctx, argc as usize, argv);
                    let args = try_ffi!(ctx.ctx, A::from_js_args(ctx, multi));
                    let value = try_ffi!(ctx.ctx, func(ctx, this, args));
                    let value = try_ffi!(ctx.ctx, value.into_js(ctx));
                    value.into_js_value()
                }),
            )
        }),
    }
}

pub unsafe extern "C" fn cb_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let rt_opaque: *mut Opaque = qjs::JS_GetRuntimeOpaque(rt) as *mut _;
    let class_id = (*rt_opaque).func_class;
    let fn_opaque = qjs::JS_GetOpaque(val, class_id) as *mut FuncOpaque;
    let fn_opaque: Box<FuncOpaque> = Box::from_raw(fn_opaque);
    mem::drop(fn_opaque);
}
