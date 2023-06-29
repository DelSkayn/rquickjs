use crate::{atom::PredefinedAtom, class::Class, qjs, Ctx, FromJs, IntoJs, Object, Result, Value};

mod args;
mod ffi;
mod into_func;
mod params;
mod types;

pub use args::{Args, IntoArg, IntoArgs};
pub use ffi::{RustFunction, StaticJsFn};
pub use params::{FromParam, FromParams, ParamReq, Params, ParamsAccessor};
pub use types::{Exhaustive, Flat, Mut, Null, Once, Opt, Rest, This, ThisFunc};

/// A trait for converting a rust function to a javascript function.
pub trait ToJsFunction<'js, P> {
    fn param_requirements() -> ParamReq;

    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js>;
}

/// A trait for functions callable from javascript but static,
/// Used for implementing callable objects.
pub trait JsFunction<'js> {
    fn call<'a>(&self, params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A trait for functions callable from javascript but static,
/// Used for implementing callable objects.
pub trait StaticJsFunction {
    fn call<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A javascript function.
#[derive(Clone, Debug)]
pub struct Function<'js>(pub(crate) Value<'js>);

impl<'js> Function<'js> {
    /// Create a new function from any type which implements `ToJsFunction`.
    pub fn new<P, F>(ctx: Ctx<'js>, f: F) -> Result<Self>
    where
        F: ToJsFunction<'js, P>,
    {
        let cls = Class::instance(ctx, RustFunction(f.to_js_function()))?;
        assert!(dbg!(cls.is_function()));
        Function(cls.into_object().into_value()).with_length(F::param_requirements().max())
    }

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

    /// Set the `name` property of this function
    pub fn set_name<S: AsRef<str>>(&self, name: S) -> Result<()> {
        let name = name.as_ref().into_js(self.0.ctx)?;
        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                PredefinedAtom::Name as qjs::JSAtom,
                name.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
        };
        Ok(())
    }

    /// Set the `name` property of this function and then return self.
    pub fn with_name<S: AsRef<str>>(self, name: S) -> Result<Self> {
        self.set_name(name)?;
        Ok(self)
    }

    /// Sets the `length` property of the function.
    pub fn set_length(&self, len: usize) -> Result<()> {
        let len = len.into_js(self.0.ctx)?;
        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                PredefinedAtom::Length as qjs::JSAtom,
                len.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
        };
        Ok(())
    }

    /// Sets the `length` property of the function and return self.
    pub fn with_length(self, len: usize) -> Result<Self> {
        self.set_length(len)?;
        Ok(self)
    }

    /// Returns the prototype which all javascript function by default have as its prototype, i.e.
    /// `Function.prototype`.
    pub fn prototype(ctx: Ctx<'js>) -> Object<'js> {
        let res = unsafe {
            Value::from_js_value(
                ctx,
                qjs::JS_DupValue(qjs::JS_GetFunctionProto(ctx.as_ptr())),
            )
        };
        // as far is I know this should always be an object.
        res.into_object()
            .expect("`Function.prototype` wasn't an object")
    }

    /// Returns wether this function is an constructor.
    pub fn is_constructor(&self) -> bool {
        let res = unsafe { qjs::JS_IsConstructor(self.ctx().as_ptr(), self.0.as_js_value()) };
        res != 0
    }

    /// Set whether this function is a constructor or not.
    pub fn set_constructor(&self, is_constructor: bool) {
        unsafe {
            qjs::JS_SetConstructorBit(
                self.ctx().as_ptr(),
                self.0.as_js_value(),
                is_constructor as i32,
            )
        };
    }

    /// Set whether this function is a constructor or not then return self.
    pub fn with_constructor(self, is_constructor: bool) -> Self {
        self.set_constructor(is_constructor);
        self
    }
}
