use std::panic::AssertUnwindSafe;

use crate::{
    class::{Class, ClassId, JsClass, Readable, Trace, Tracer},
    qjs,
    value::function::{Params, StaticJsFunction},
    FromJs, Function, Outlive, Result, Value,
};
pub use mac::static_fn;

use super::JsFunction;

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
    let ctx = args.ctx();

    ctx.handle_panic(AssertUnwindSafe(|| {
        let value = F::call(args)
            .map(Value::into_js_value)
            .unwrap_or_else(|error| error.throw(ctx));
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
/// You can quickly create an ClassFn from any function by using the [`class_fn`] macro.
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

/// The class used for wrapping closures, all rust functions which are callable from javascript are
/// instances of this class.
pub struct RustFunction<'js>(pub Box<dyn JsFunction<'js> + 'js>);

/// The static function which is called when javascripts calls an instance of RustFunction
fn call_rust_func_class<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>> {
    let this = Class::<RustFunction>::from_js(params.ctx(), params.function())?;
    // RustFunction isn't readable this always succeeds.
    let borrow = this.borrow();
    (*borrow).0.call(params)
}

unsafe impl<'js> Outlive<'js> for RustFunction<'js> {
    type Target<'to> = RustFunction<'to>;
}

impl<'js> Trace<'js> for RustFunction<'js> {
    fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
}

impl<'js> JsClass<'js> for RustFunction<'js> {
    const NAME: &'static str = "RustFunction";

    type Mutable = Readable;

    fn class_id() -> &'static crate::class::ClassId {
        static ID: ClassId = ClassId::new();
        &ID
    }

    fn prototype(ctx: crate::Ctx<'js>) -> Result<Option<crate::Object<'js>>> {
        Ok(Some(Function::prototype(ctx)))
    }

    fn function() -> Option<StaticJsFn> {
        Some(static_fn!(call_rust_func_class))
    }
}

mod mac {
    /// A macro for implementing StaticJsFunction for generic functions.
    #[macro_export]
    macro_rules! static_fn {
        ($f:ident) => {{
            pub struct CarryFunction;
            impl $crate::function::StaticJsFunction for CarryFunction {
                fn call<'a, 'js>(
                    params: $crate::function::Params<'a, 'js>,
                ) -> $crate::Result<$crate::Value<'js>> {
                    $f(params)
                }
            }
            StaticJsFn::new::<CarryFunction>()
        }};
    }
    pub use static_fn;
}
