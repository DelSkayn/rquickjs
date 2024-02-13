use crate::{
    function::{Exhaustive, Flat, FuncArg, Opt, Rest, This},
    qjs, Ctx, FromJs, Result, Value,
};
use std::slice;

/// A struct which contains the values a callback is called with.
///
/// Arguments retrieved from the JavaScript side for calling Rust functions.
pub struct Params<'a, 'js> {
    ctx: Ctx<'js>,
    function: qjs::JSValue,
    this: qjs::JSValue,
    args: &'a [qjs::JSValue],
    is_constructor: bool,
}

impl<'a, 'js> Params<'a, 'js> {
    /// Create params from the arguments returned by the class callback.
    pub(crate) unsafe fn from_ffi_class(
        ctx: *mut qjs::JSContext,
        function: qjs::JSValue,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
        _flags: qjs::c_int,
    ) -> Self {
        let args = if argv.is_null() {
            let argc = usize::try_from(argc).expect("invalid argument number");
            slice::from_raw_parts(argv, argc)
        } else {
            assert_eq!(
                argc, 0,
                "got a null pointer from quickjs for a non-zero number of args"
            );
            [].as_slice()
        };

        Self {
            ctx: Ctx::from_ptr(ctx),
            function,
            this,
            args,
            is_constructor: false,
        }
    }

    /// Checks if the parameters fit the param num requirements.
    pub fn check_params(&self, num: ParamRequirement) -> Result<()> {
        if self.args.len() < num.min {
            return Err(crate::Error::MissingArgs {
                expected: num.min,
                given: self.args.len(),
            });
        }
        if num.exhaustive && self.args.len() > num.max {
            return Err(crate::Error::TooManyArgs {
                expected: num.max,
                given: self.args.len(),
            });
        }
        Ok(())
    }

    /// Returns the context associated with call.
    pub fn ctx(&self) -> &Ctx<'js> {
        &self.ctx
    }

    /// Returns the value on which this function called. i.e. in `bla.foo()` the `foo` value.
    pub fn function(&self) -> Value<'js> {
        unsafe { Value::from_js_value_const(self.ctx.clone(), self.function) }
    }

    /// Returns the this on which this function called. i.e. in `bla.foo()` the `bla` value.
    pub fn this(&self) -> Value<'js> {
        unsafe { Value::from_js_value_const(self.ctx.clone(), self.this) }
    }

    /// Returns the argument at a given index..
    pub fn arg(&self, index: usize) -> Option<Value<'js>> {
        self.args
            .get(index)
            .map(|arg| unsafe { Value::from_js_value_const(self.ctx.clone(), *arg) })
    }

    /// Returns the number of arguments.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Returns if there are no arguments
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Returns if the function is called as a constructor.
    ///
    /// If it is the value return by `this` is actually the `new.target` value.
    pub fn is_constructor(&self) -> bool {
        self.is_constructor
    }

    /// Turns the params into an accessor object for extracting the arguments.
    pub fn access(self) -> ParamsAccessor<'a, 'js> {
        ParamsAccessor {
            params: self,
            offset: 0,
        }
    }
}

/// Accessor to parameters used for retrieving arguments in order one at the time.
pub struct ParamsAccessor<'a, 'js> {
    params: Params<'a, 'js>,
    offset: usize,
}

impl<'a, 'js> ParamsAccessor<'a, 'js> {
    /// Returns the context associated with the params.
    pub fn ctx(&self) -> &Ctx<'js> {
        self.params.ctx()
    }

    /// Returns this value of call from which the params originate.
    pub fn this(&self) -> Value<'js> {
        self.params.this()
    }

    /// Returns the value on which this function called. i.e. in `bla.foo()` the `foo` value.
    pub fn function(&self) -> Value<'js> {
        self.params.function()
    }

    /// Returns the next arguments.
    ///
    /// Each call to this function returns a different argument
    ///
    /// # Panic
    /// This function panics if it is called more times then there are arguments.
    pub fn arg(&mut self) -> Value<'js> {
        assert!(
            self.offset < self.params.args.len(),
            "arg called too many times"
        );
        let res = self.params.args[self.offset];
        self.offset += 1;
        // TODO: figure out ownership
        unsafe { Value::from_js_value_const(self.params.ctx.clone(), res) }
    }

    /// returns the number of arguments remaining
    pub fn len(&self) -> usize {
        self.params.args.len() - self.offset
    }
    /// returns whether there are any arguments remaining.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A struct encoding the requirements of a parameter set.
pub struct ParamRequirement {
    min: usize,
    max: usize,
    exhaustive: bool,
}

impl ParamRequirement {
    /// Returns the requirement of a single required parameter
    pub const fn single() -> Self {
        ParamRequirement {
            min: 1,
            max: 1,
            exhaustive: false,
        }
    }

    /// Makes the requirements exhaustive i.e. the parameter set requires that the function is
    /// called with no arguments than parameters
    pub const fn exhaustive() -> Self {
        ParamRequirement {
            min: 0,
            max: 0,
            exhaustive: true,
        }
    }

    /// Returns the requirements for a single optional parameter
    pub const fn optional() -> Self {
        ParamRequirement {
            min: 0,
            max: 1,
            exhaustive: false,
        }
    }

    /// Returns the requirements for a any number of parameters
    pub const fn any() -> Self {
        ParamRequirement {
            min: 0,
            max: usize::MAX,
            exhaustive: false,
        }
    }

    /// Returns the requirements for no parameters
    pub const fn none() -> Self {
        ParamRequirement {
            min: 0,
            max: 0,
            exhaustive: false,
        }
    }

    /// Combine to requirements into one which covers both.
    pub const fn combine(self, other: Self) -> ParamRequirement {
        Self {
            min: self.min.saturating_add(other.min),
            max: self.max.saturating_add(other.max),
            exhaustive: self.exhaustive || other.exhaustive,
        }
    }

    /// Returns the minimum number of arguments for this requirement
    pub fn min(&self) -> usize {
        self.min
    }

    /// Returns the maximum number of arguments for this requirement
    pub fn max(&self) -> usize {
        self.max
    }

    /// Returns whether this function is required to be exhaustive called
    ///
    /// i.e. there can be no more arguments then parameters.
    pub fn is_exhaustive(&self) -> bool {
        self.exhaustive
    }
}

/// A trait to extract argument values.
pub trait FromParam<'js>: Sized {
    /// The parameters requirements this value requires.
    fn param_requirement() -> ParamRequirement;

    /// Convert from a parameter value.
    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self>;
}

impl<'js, T: FromJs<'js>> FromParam<'js> for T {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::single()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        let ctx = params.ctx().clone();
        T::from_js(&ctx, params.arg())
    }
}

impl<'js> FromParam<'js> for Ctx<'js> {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::none()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        Ok(params.ctx().clone())
    }
}

impl<'js, T: FromJs<'js>> FromParam<'js> for Opt<T> {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::optional()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        if !params.is_empty() {
            let ctx = params.ctx().clone();
            Ok(Opt(Some(T::from_js(&ctx, params.arg())?)))
        } else {
            Ok(Opt(None))
        }
    }
}

impl<'js, T: FromJs<'js>> FromParam<'js> for This<T> {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        T::from_js(params.ctx(), params.this()).map(This)
    }
}

impl<'js, T: FromJs<'js>> FromParam<'js> for FuncArg<T> {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        T::from_js(params.ctx(), params.function()).map(FuncArg)
    }
}

impl<'js, T: FromJs<'js>> FromParam<'js> for Rest<T> {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        let mut res = Vec::with_capacity(params.len());
        for _ in 0..params.len() {
            let p = params.arg();
            res.push(T::from_js(params.ctx(), p)?);
        }
        Ok(Rest(res))
    }
}

impl<'js, T: FromParams<'js>> FromParam<'js> for Flat<T> {
    fn param_requirement() -> ParamRequirement {
        T::param_requirements()
    }

    fn from_param<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        T::from_params(params).map(Flat)
    }
}

impl<'js> FromParam<'js> for Exhaustive {
    fn param_requirement() -> ParamRequirement {
        ParamRequirement::exhaustive()
    }

    fn from_param<'a>(_params: &mut ParamsAccessor<'a, 'js>) -> Result<Self> {
        Ok(Exhaustive)
    }
}

/// A trait to extract a tuple of argument values.
pub trait FromParams<'js>: Sized {
    /// The parameters requirements this value requires.
    fn param_requirements() -> ParamRequirement;

    /// Convert from a parameter value.
    fn from_params<'a>(params: &mut ParamsAccessor<'a, 'js>) -> Result<Self>;
}

macro_rules! impl_from_params{
    ($($t:ident),*) => {
        #[allow(non_snake_case)]
        impl<'js $(,$t)*> FromParams<'js> for ($($t,)*)
        where
            $($t : FromParam<'js>,)*
        {

            fn param_requirements() -> ParamRequirement{
                ParamRequirement::none()
                    $(.combine($t::param_requirement()))*
            }

            fn from_params<'a>(_args: &mut ParamsAccessor<'a,'js>) -> Result<Self>{
                Ok((
                    $($t::from_param(_args)?,)*
                ))
            }
        }
    };
}

impl_from_params!();
impl_from_params!(A);
impl_from_params!(A, B);
impl_from_params!(A, B, C);
impl_from_params!(A, B, C, D);
impl_from_params!(A, B, C, D, E);
impl_from_params!(A, B, C, D, E, F);
impl_from_params!(A, B, C, D, E, F, G);
