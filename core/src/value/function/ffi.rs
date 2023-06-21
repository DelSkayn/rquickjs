use std::panic::AssertUnwindSafe;

use crate::{
    qjs,
    value::function::{Args, JsFunction},
    Value,
};

///. The C side callback
pub unsafe extern "C" fn js_callback<F: JsFunction>(
    ctx: *mut qjs::JSContext,
    function: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    _flags: qjs::c_int,
) -> qjs::JSValue {
    let args = Args::from_ffi(ctx, function, this, argc, argv, _flags);
    let ctx = args.ctx();

    ctx.handle_panic(AssertUnwindSafe(|| {
        let value = F::call(args)
            .map(Value::into_js_value)
            .unwrap_or_else(|error| error.throw(ctx));
        value
    }))
}
