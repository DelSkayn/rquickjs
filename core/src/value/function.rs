mod args;
mod params;
mod types;

pub use args::{Args, IntoArg, IntoArgs};
pub use params::{FromParam, FromParams, ParamReq, Params, ParamsAccessor};
pub use types::{Exhaustive, Flat, Null, Opt, Rest, This};
mod ffi;

use crate::{FromJs, Result, Value};

pub trait JsFunction {
    fn call<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>>;
}

#[derive(Clone)]
pub struct Function<'js>(pub(crate) Value<'js>);

impl<'js> Function<'js> {
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

    pub fn call_arg<R>(&self, args: Args<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        args.apply(self)
    }
}
