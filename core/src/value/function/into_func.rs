use super::{CellFn, FromParams, IntoJsFunction, JsFunction, ParamReq, Params};
use crate::{IntoJs, Result, Value};

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

impl<'js, P, R, F> IntoJsFunction<'js, F, P, R> for F
where
    F: CellFn<'js, P, R> + 'js,
    P: FromParams<'js> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn param_requirements() -> ParamReq {
        P::params_requirements()
    }

    fn into_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let params = P::from_params(&mut params.access())?;
            let r = self.call(params)?;
            r.into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}

#[cfg(feature = "futures")]
impl<'js, P, R, F, Fut> IntoJsFunction<'js, F, P, Fut> for Async<F>
where
    F: CellFn<'js, P, Fut> + 'js,
    P: FromParams<'js> + 'js,
    Fut: Future<Output = Result<R>> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn param_requirements() -> ParamReq {
        P::params_requirements()
    }

    fn into_js_function(self) -> Box<dyn JsFunction<'js> + 'js> {
        Box::new(move |params: Params<'_, 'js>| {
            let ctx = params.ctx();
            let params = P::from_params(&mut params.access())?;
            let r = self.0.call(params)?;
            Promised(r).into_js(ctx)
        }) as Box<dyn JsFunction<'js> + 'js>
    }
}
