use crate::{
    atom::PredefinedAtom,
    class::{Class, JsClass},
    function::ffi::RustFunc,
    qjs, Ctx, Error, FromJs, IntoJs, Object, Result, Value,
};

mod args;
mod ffi;
mod into_func;
mod params;
mod types;

pub use args::{Args, IntoArg, IntoArgs};
pub use ffi::{RustFunction, StaticJsFn};
pub use params::{FromParam, FromParams, ParamRequirement, Params, ParamsAccessor};
pub use types::{Exhaustive, Flat, Func, FuncArg, Mut, Null, Once, Opt, Rest, This};

/// A trait for converting a rust function to a javascript function.
pub trait IntoJsFunc<'js, P> {
    /// Returns the requirements this function has for the set of arguments used to call this
    /// function.
    fn param_requirements() -> ParamRequirement;

    /// Call the function with the given parameters.
    fn call<'a>(&self, params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A trait for functions callable from javascript but static,
/// Used for implementing callable objects.
pub trait StaticJsFunction {
    fn call<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A javascript function.
#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Function<'js>(pub(crate) Object<'js>);

impl<'js> Function<'js> {
    /// Create a new function from a rust function which implements [`IntoJsFunc`].
    pub fn new<P, F>(ctx: Ctx<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        let func =
            Box::new(move |params: Params<'_, 'js>| f.call(params)) as Box<dyn RustFunc<'js> + 'js>;

        let cls = Class::instance(ctx, RustFunction(func))?;
        debug_assert!(cls.is_function());
        Function(cls.into_object()).with_length(F::param_requirements().min())
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

    /// Defer call the function with given arguments.
    ///
    /// Calling a function with defer is equivalent to calling a javascript function with
    /// `setTimeout(func,0)`.
    pub fn defer<A, R>(&self, args: A) -> Result<R>
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

#[repr(transparent)]
pub struct Constructor<'js>(pub(crate) Function<'js>);

impl<'js> Constructor<'js> {
    /// Creates a rust constructor function for a rust class.
    ///
    /// Note that this function creates a constructor from a given function, the returned constructor
    /// is thus not the same as the one returned from [`JsClass::constructor`].
    pub fn new_class<C, F, P>(ctx: Ctx<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
        C: JsClass<'js>,
    {
        let func = Box::new(move |params: Params<'_, 'js>| -> Result<Value<'js>> {
            let this = params.this();
            let proto = this
                .into_function()
                .map(|func| func.get(PredefinedAtom::Prototype))
                .unwrap_or_else(|| Ok(Class::<C>::prototype(ctx)))?;

            let res = f.call(params)?;
            res.as_object()
                .ok_or_else(|| Error::IntoJs {
                    from: res.type_of().as_str(),
                    to: "object",
                    message: Some("rust constructor function did not return a object".to_owned()),
                })?
                .set_prototype(proto.as_ref())?;
            Ok(res)
        });
        let func = Function(Class::instance(ctx, RustFunction(func))?.into_object())
            .with_constructor(true);
        unsafe {
            qjs::JS_SetConstructor(
                ctx.as_ptr(),
                func.as_js_value(),
                Class::<C>::prototype(ctx)
                    .as_ref()
                    .map(|x| x.as_js_value())
                    .unwrap_or(qjs::JS_NULL),
            )
        };
        Ok(Constructor(func))
    }

    /// Create a new rust constructor function with a given prototype.
    ///
    /// Usefull if the function does not return a rust class.
    pub fn new_prototype<F, P>(ctx: Ctx<'js>, prototype: Object<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        let proto_clone = prototype.clone();
        let func = Box::new(move |params: Params<'_, 'js>| -> Result<Value<'js>> {
            let this = params.this();
            let proto = this
                .as_function()
                .map(|func| func.get(PredefinedAtom::Prototype))
                .unwrap_or_else(|| Ok(Some(proto_clone.clone())))?;

            let res = f.call(params)?;
            res.as_object()
                .ok_or_else(|| Error::IntoJs {
                    from: res.type_of().as_str(),
                    to: "object",
                    message: Some("rust constructor function did not return a object".to_owned()),
                })?
                .set_prototype(proto.as_ref())?;
            Ok(res)
        });
        let func = Function(Class::instance(ctx, RustFunction(func))?.into_object())
            .with_constructor(true);
        unsafe {
            qjs::JS_SetConstructor(ctx.as_ptr(), func.as_js_value(), prototype.as_js_value())
        };
        Ok(Constructor(func))
    }

    pub fn construct<A, R>(&self, args: A) -> Result<R>
    where
        A: IntoArgs<'js>,
        R: FromJs<'js>,
    {
        let ctx = self.0.ctx;
        let num = args.num_args();
        let mut accum_args = Args::new(ctx, num);
        args.into_args(&mut accum_args)?;
        self.construct_args(accum_args)
    }

    pub fn construct_args<R>(&self, args: Args<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        args.construct(self)
    }
}
