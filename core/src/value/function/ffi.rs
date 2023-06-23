use std::panic::AssertUnwindSafe;

use crate::{
    class::{Class, ClassId, JsClass, Readable},
    qjs,
    value::function::{JsFunction, Params, StaticJsFunction},
    FromJs, Result, Value,
};
pub use mac::class_fn;

///. The C side callback
pub unsafe extern "C" fn js_callback<F: StaticJsFunction>(
    ctx: *mut qjs::JSContext,
    function: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    _flags: qjs::c_int,
) -> qjs::JSValue {
    let args = Params::from_ffi(ctx, function, this, argc, argv, _flags);
    let ctx = args.ctx();

    ctx.handle_panic(AssertUnwindSafe(|| {
        let value = F::call(args)
            .map(Value::into_js_value)
            .unwrap_or_else(|error| error.throw(ctx));
        value
    }))
}

/// A static class method for making function object like classes.
///
/// You can quickly create an ClassFn from any function by using the [`class_fn`] macro.
pub struct ClassFn(
    pub(crate)  unsafe extern "C" fn(
        *mut qjs::JSContext,
        qjs::JSValue,
        qjs::JSValue,
        qjs::c_int,
        *mut qjs::JSValue,
        qjs::c_int,
    ) -> qjs::JSValue,
);

impl ClassFn {
    /// Create a new class fn object from a type implementing the [`StaticJsFunction`] trait.
    pub fn new<F: StaticJsFunction>() -> Self {
        Self(js_callback::<F>)
    }
}

pub struct RustFunction(Box<dyn JsFunction>);

fn call_rust_func<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>> {
    let this = Class::<RustFunction>::from_js(params.ctx(), params.function())?;
    let borrow = this.borrow();
    (*borrow).0.call(params)
}

unsafe impl JsClass for RustFunction {
    const NAME: &'static str = "RustFunction";

    type Mutable = Readable;

    type Outlive<'a> = RustFunction;

    fn class_id() -> &'static crate::class::ClassId {
        static ID: ClassId = ClassId::new();
        &ID
    }

    fn prototype<'js>(ctx: crate::Ctx<'js>) -> Result<Option<crate::Object<'js>>> {
        todo!()
    }

    fn function() -> Option<ClassFn> {
        Some(class_fn!(call_rust_func))
    }
}

mod mac {
    /// A macro for implementing StaticJsFunction for generic functions.
    #[macro_export]
    macro_rules! class_fn {
        ($f:ident) => {{
            pub struct CarryFunction;
            impl $crate::function::StaticJsFunction for CarryFunction {
                fn call<'a, 'js>(
                    params: $crate::function::Params<'a, 'js>,
                ) -> $crate::Result<$crate::Value<'js>> {
                    $f(params)
                }
            }
            ClassFn::new::<CarryFunction>()
        }};
    }
    pub use class_fn;
}
