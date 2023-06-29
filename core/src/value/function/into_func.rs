use super::{CellFn, FromParams, JsFunction, ParamReq, Params, ToJsFunction};
use crate::{FromJs, Function, IntoJs, Result, Value};

#[cfg(feature = "futures")]
use crate::{function::types::Async, promise::Promised};
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

impl<'js, R, P, F> ToJsFunction<'js, P, false> for F
where
    F: CellFn<'js, P, false, Output = R> + 'js,
    P: FromParams<'js> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        P::params_required()
    }

    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let params = P::from_params(&mut params.access())?;
            let r = self.call(params)?;
            r.into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

#[cfg(feature = "futures")]
impl<'js, P, R, F, Fut> ToJsFunction<'js, P, true> for F
where
    F: CellFn<'js, P, true, Output = Fut> + 'js,
    P: FromParams<'js> + 'js,
    Fut: Future<Output = Result<R>> + 'js,
    R: IntoJs<'js> + 'js,
{
    const NAME: Option<&'static str> = None;
    const CONSTRUCTOR: Option<bool> = None;

    fn param_requirements() -> ParamReq {
        P::params_required()
    }

    fn to_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let params = P::from_params(&mut params.access())?;
            let r = self.call(params)?;
            Promised(r).into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}
