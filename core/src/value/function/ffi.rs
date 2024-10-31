use crate::{
    class::{JsCell, JsClass, Readable, Trace, Tracer},
    qjs,
    value::function::Params,
    Ctx, Function, JsLifetime, Object, Result, Value,
};

use super::Constructor;

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
