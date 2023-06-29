use super::{FromParams, JsFunction, ParamReq, Params, ToJsFunction};
use crate::{IntoJs, Result, Value};

#[cfg(feature = "futures")]
use crate::{
    function::types::{Async, Mut, Once},
    promise::Promised,
    result::{BorrowError, Error},
};
#[cfg(feature = "futures")]
use std::future::Future;

impl<'js, F> JsFunction<'js> for F
where
    F: for<'a> Fn(Params<'a, 'js>) -> Result<Value<'js>> + 'js,
{
    fn call<'a>(&self, params: Params<'a, 'js>) -> crate::Result<Value<'js>> {
        (self)(params)
    }
}

macro_rules! impl_to_js_function {
    ($($t:ident),*$(,)?) => {

impl<'js, R, Fun $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Fun
where
    Fun: Fn($($t),*) -> R + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let r = (self)($($t),*);
            r.into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

#[cfg(feature = "futures")]
impl<'js, R, Fun, Fut $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Async<Fun>
where
    Fun: Fn($($t),*) -> Fut + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    Fut: Future<Output = R> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let fut = (self.0)($($t),*);
            Promised(fut).into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}


impl<'js, R, Fun $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Mut<Fun>
where
    Fun: Fn($($t),*) -> R + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let mut lock = self.0.try_borrow_mut().map_err(|_| Error::FunctionBorrow(BorrowError::AlreadyBorrowed))?;
            let r = (lock)($($t),*);
            r.into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

#[cfg(feature = "futures")]
impl<'js, R, Fun, Fut $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Async<Mut<Fun>>
where
    Fun: Fn($($t),*) -> Fut + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    Fut: Future<Output = R> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let mut lock = self.0.0.try_borrow_mut().map_err(|_| Error::FunctionBorrow(BorrowError::AlreadyBorrowed))?;
            let fut = (lock)($($t),*);
            Promised(fut).into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

impl<'js, R, Fun $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Once<Fun>
where
    Fun: Fn($($t),*) -> R + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let mut lock = self.0.take().ok_or(Error::FunctionBorrow(BorrowError::AlreadyUsed))?;
            let r = (lock)($($t),*);
            r.into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

#[cfg(feature = "futures")]
impl<'js, R, Fun, Fut $(,$t)*> ToJsFunction<'js, ($($t,)*)> for Async<Once<Fun>>
where
    Fun: Fn($($t),*) -> Fut + 'js,
    ($($t,)*): FromParams<'js> + 'js,
    Fut: Future<Output = R> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        <($($t,)*)>::params_required()
    }

    #[allow(non_snake_case)]
    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
            let mut lock = self.0.0.take().ok_or(Error::FunctionBorrow(BorrowError::AlreadyUsed))?;
            let fut = (lock)($($t),*);
            Promised(fut).into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}


    };
}

impl_to_js_function!();
impl_to_js_function!(A);
impl_to_js_function!(A, B);
impl_to_js_function!(A, B, C);
impl_to_js_function!(A, B, C, D);
impl_to_js_function!(A, B, C, D, E);
impl_to_js_function!(A, B, C, D, E, F);
impl_to_js_function!(A, B, C, D, E, F, G);
impl_to_js_function!(A, B, C, D, E, F, G, H);
