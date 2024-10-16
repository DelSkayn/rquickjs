use std::panic::AssertUnwindSafe;

use crate::{
    class::{JsCell, JsClass, Readable, Trace, Tracer},
    qjs,
    value::function::{Params, StaticJsFunction},
    Ctx, Function, JsLifetime, Object, Result, Value,
};

use super::Constructor;

///. The C side callback
pub unsafe extern "C" fn js_callback_class<F: StaticJsFunction>(
    ctx: *mut qjs::JSContext,
    function: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    _flags: qjs::c_int,
) -> qjs::JSValue {
    let args = Params::from_ffi_class(ctx, function, this, argc, argv, _flags);
    let ctx = args.ctx().clone();

    ctx.handle_panic(AssertUnwindSafe(|| {
        let value = F::call(args)
            .map(Value::into_js_value)
            .unwrap_or_else(|error| error.throw(&ctx));
        value
    }))
}

pub unsafe extern "C" fn defer_call_job(
    ctx: *mut qjs::JSContext,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
) -> qjs::JSValue {
    let func = *argv.offset((argc - 1) as _);
    let this = *argv.offset((argc - 2) as _);
    let argc = argc - 2;
    qjs::JS_Call(ctx, func, this, argc, argv)
}

/// A static class method for making function object like classes.
///
/// You can quickly create an `StaticJsFn` from any function by using the [`static_fn`] macro.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct StaticJsFn(
    pub(crate)  unsafe extern "C" fn(
        *mut qjs::JSContext,
        qjs::JSValue,
        qjs::JSValue,
        qjs::c_int,
        *mut qjs::JSValue,
        qjs::c_int,
    ) -> qjs::JSValue,
);

impl StaticJsFn {
    /// Create a new class fn object from a type implementing the [`StaticJsFunction`] trait.
    pub fn new<F: StaticJsFunction>() -> Self {
        Self(js_callback_class::<F>)
    }
}

/// A trait for dynamic callbacks to Rust.
pub trait RustFunc<'js> {
    /// Call the actual function with a given set of parameters and return a function.
    fn call<'a>(&self, params: Params<'a, 'js>) -> Result<Value<'js>>;
}

impl<'js, F> RustFunc<'js> for F
where
    for<'a> F: Fn(Params<'a, 'js>) -> Result<Value<'js>>,
{
    fn call<'a>(&self, params: Params<'a, 'js>) -> Result<Value<'js>> {
        (self)(params)
    }
}

/// The class used for wrapping closures, rquickjs implements callbacks by creating an instances of
/// this class.
pub struct RustFunction<'js>(pub Box<dyn RustFunc<'js> + 'js>);

unsafe impl<'js> JsLifetime<'js> for RustFunction<'js> {
    type Changed<'to> = RustFunction<'to>;
}

impl<'js> Trace<'js> for RustFunction<'js> {
    fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
}

impl<'js> JsClass<'js> for RustFunction<'js> {
    const NAME: &'static str = "RustFunction";

    type Mutable = Readable;

    const CALLABLE: bool = true;

    fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        Ok(Some(Function::prototype(ctx.clone())))
    }

    fn constructor(_ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        Ok(None)
    }

    fn call<'a>(this: &JsCell<'js, Self>, params: Params<'a, 'js>) -> Result<Value<'js>> {
        this.borrow().0.call(params)
    }
}
