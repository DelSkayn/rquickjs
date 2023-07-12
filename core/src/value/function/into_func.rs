use super::{FromParams, IntoJsFunc, ParamRequirement, Params};
use crate::{
    function::types::{MutFn, OnceFn},
    result::{BorrowError, Error},
    IntoJs, Result, Value,
};

#[cfg(feature = "futures")]
use crate::{function::types::Async, promise::Promised};
#[cfg(feature = "futures")]
use std::future::Future;

macro_rules! impl_to_js_function {
    ($($t:ident),*$(,)?) => {
        impl<'js, R, Fun $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for Fun
        where
            Fun: Fn($($t),*) -> R + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let r = (self)($($t),*);
                r.into_js(&ctx)
            }
        }

        #[cfg(feature = "futures")]
        impl<'js, R, Fun, Fut $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for Async<Fun>
        where
            Fun: Fn($($t),*) -> Fut + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            Fut: Future<Output = R> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let fut = (self.0)($($t),*);
                Promised(fut).into_js(&ctx)
            }
        }


        impl<'js, R, Fun $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for MutFn<Fun>
        where
            Fun: FnMut($($t),*) -> R + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let mut lock = self.0.try_borrow_mut().map_err(|_| Error::FunctionBorrow(BorrowError::AlreadyBorrowed))?;
                let r = (lock)($($t),*);
                r.into_js(&ctx)
            }
        }

        #[cfg(feature = "futures")]
        impl<'js, R, Fun, Fut $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for Async<MutFn<Fun>>
        where
            Fun: FnMut($($t),*) -> Fut + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            Fut: Future<Output = R> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let mut lock = self.0.0.try_borrow_mut().map_err(|_| Error::FunctionBorrow(BorrowError::AlreadyBorrowed))?;
                let fut = (lock)($($t),*);
                Promised(fut).into_js(&ctx)
            }
        }

        impl<'js, R, Fun $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for OnceFn<Fun>
        where
            Fun: FnOnce($($t),*) -> R + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let lock = self.0.take().ok_or(Error::FunctionBorrow(BorrowError::AlreadyUsed))?;
                let r = (lock)($($t),*);
                r.into_js(&ctx)
            }
        }

        #[cfg(feature = "futures")]
        impl<'js, R, Fun, Fut $(,$t)*> IntoJsFunc<'js, ($($t,)*)> for Async<OnceFn<Fun>>
        where
            Fun: FnOnce($($t),*) -> Fut + 'js,
            ($($t,)*): FromParams<'js> + 'js,
            Fut: Future<Output = R> + 'js,
            R: IntoJs<'js> + 'js,
        {

            fn param_requirements() -> ParamRequirement {
                <($($t,)*)>::param_requirements()
            }

            #[allow(non_snake_case)]
            fn call(&self, params: Params<'_, 'js>) -> Result<Value<'js>> {
                let ctx = params.ctx().clone();
                let ($($t,)*) = <($($t,)*)>::from_params(&mut params.access())?;
                let lock = self.0.0.take().ok_or(Error::FunctionBorrow(BorrowError::AlreadyUsed))?;
                let fut = (lock)($($t),*);
                Promised(fut).into_js(&ctx)
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
