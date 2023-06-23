use crate::{qjs, FromJs, Result, Value};

mod args;
mod ffi;
mod params;
mod types;

pub use args::{Args, IntoArg, IntoArgs};
pub use ffi::ClassFn;
pub use params::{FromParam, FromParams, ParamReq, Params, ParamsAccessor};
pub use types::{Exhaustive, Flat, Func, Null, Opt, Rest, This};

/// A trait for functions callable from javascript.
pub trait JsFunction {
    fn call<'a, 'js>(&self, params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A trait for functions callable from javascript but static,
/// Used for implementing callable objects.
pub trait StaticJsFunction {
    fn call<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>>;
}

#[derive(Clone)]
pub struct Function<'js>(pub(crate) Value<'js>);

impl<'js> Function<'js> {
    /// Call the function with given arguments.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: IntoArgs<'js>,
        R: FromJs<'js>,
    {
        let ctx = self.0.ctx;
        let num = args.num_args();
        let mut accum_args = Args::new(ctx, num);
        args.into_args(&mut accum_args)?;
        self.call_arg(accum_args)
    }

    /// Call the function with given arguments in the form of an [`Args`] object.
    pub fn call_arg<R>(&self, args: Args<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        args.apply(self)
    }

    /// Returns wether this function is an constructor.
    pub fn is_constructor(&self) -> bool {
        let res = unsafe { qjs::JS_IsConstructor(self.ctx().as_ptr(), self.0.as_js_value()) };
        res != 0
    }

    /// Make this function an constructor.
    pub fn set_constructor(&self, is_constructor: bool) {
        unsafe {
            qjs::JS_SetConstructorBit(
                self.ctx().as_ptr(),
                self.0.as_js_value(),
                is_constructor as i32,
            )
        };
    }
}
