use super::{
    FromJs, FromJsMulti, IntoJs, IntoJsMulti, MultiValue, MultiValueJs, RestValues, Value,
};
use crate::{Ctx, Result};
use std::iter::{empty, once};

impl<'js> IntoJsMulti<'js> for MultiValueJs<'js> {
    fn to_js_multi(self, _: Ctx<'js>) -> Result<MultiValueJs<'js>> {
        Ok(self)
    }
}

impl<'js, T: IntoJs<'js>> IntoJsMulti<'js> for T {
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<MultiValueJs<'js>> {
        Ok(vec![self.to_js(ctx)?].into())
    }
}

impl<'js> FromJsMulti<'js> for MultiValue<'js> {
    fn from_js_multi(_: Ctx<'js>, value: MultiValue<'js>) -> Result<Self> {
        Ok(value)
    }

    const LEN: i32 = -1;
}

impl<'js> FromJsMulti<'js> for () {
    fn from_js_multi(_: Ctx<'js>, _: MultiValue<'js>) -> Result<Self> {
        Ok(())
    }

    const LEN: i32 = 0;
}

impl<'js, X> IntoJsMulti<'js> for RestValues<X>
where
    X: IntoJs<'js>,
{
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<MultiValueJs<'js>> {
        let rest: Vec<_> = self.into();
        let iter = rest.into_iter().map(|value| value.to_js(ctx));
        Ok(iter.collect::<Result<Vec<_>>>()?.into())
    }
}

impl<'js, X> FromJsMulti<'js> for RestValues<X>
where
    X: FromJs<'js>,
{
    fn from_js_multi(ctx: Ctx<'js>, mut value: MultiValue<'js>) -> Result<Self> {
        Ok(value
            .iter()
            .map(|value| X::from_js(ctx, value))
            .collect::<Result<Vec<_>>>()?
            .into())
    }

    const LEN: i32 = 0;
}

macro_rules! impl_from_to_js_multi {
    ($($($t:ident)*;)*) => {
        $(
            impl<'js, $($t,)*> IntoJsMulti<'js> for ($($t,)*)
            where
                $($t: IntoJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn to_js_multi(self, ctx: Ctx<'js>) -> Result<MultiValueJs<'js>>{
                    let ($($t,)*) = self;
                    Ok(vec![
                        $($t.to_js(ctx)?,)*
                    ].into())
                }
            }

            impl<'js, $($t,)* X> IntoJsMulti<'js> for ($($t,)* RestValues<X>)
            where
                $($t: IntoJs<'js>,)*
                X: IntoJs<'js>,
            {
                #[allow(non_snake_case)]
                fn to_js_multi(self, ctx: Ctx<'js>) -> Result<MultiValueJs<'js>>{
                    let ($($t,)* X) = self;
                    let iter = empty();
                    $(let iter = iter.chain(once($t.to_js(ctx))));*;
                    let rest: Vec<_> = X.into();
                    let iter = iter.chain(rest.into_iter().map(|value| value.to_js(ctx)));
                    Ok(iter.collect::<Result<Vec<_>>>()?.into())
                }
            }

            impl<'js, $($t,)*> FromJsMulti<'js> for ($($t,)*)
            where
                $($t: FromJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn from_js_multi(ctx: Ctx<'js>, mut value: MultiValue<'js>) -> Result<Self> {
                    let mut iter = value.iter();
                    Ok((
                        $({
                            let v = iter.next()
                                .unwrap_or(Value::Undefined);
                            $t::from_js(ctx,v)?
                        },)*
                    ))
                }

                const LEN: i32 = impl_from_to_js_multi!(@count $($t),*);
            }

            impl<'js, $($t,)* X> FromJsMulti<'js> for ($($t,)* RestValues<X>)
            where
                $($t: FromJs<'js>,)*
                X: FromJs<'js>,
            {
                #[allow(non_snake_case)]
                fn from_js_multi(ctx: Ctx<'js>, mut value: MultiValue<'js>) -> Result<Self> {
                    let mut iter = value.iter();
                    Ok((
                        $({
                            let value = iter.next()
                                .unwrap_or_else(|| Value::Undefined);
                            $t::from_js(ctx, value)?
                        },)*
                        iter.map(|value| X::from_js(ctx, value)).collect::<Result<Vec<_>>>()?.into()
                    ))
                }

                const LEN: i32 = impl_from_to_js_multi!(@count $($t),*);
            }
        )*
    };

    (@count $($t:ident),*) => {
        0 $(+ impl_from_to_js_multi!(@1 $t))*
    };

    (@1 $($t:tt)*) => { 1 };
}

impl_from_to_js_multi! {
    A;
    A B;
    A B C;
    A B C D;
    A B C D E;
    A B C D E F;
    A B C D E F G;
    A B C D E F G H;
    A B C D E F G H I;
    A B C D E F G H I J;
    A B C D E F G H I J K;
    A B C D E F G H I J K L;
    A B C D E F G H I J K L M;
    A B C D E F G H I J K L M N;
    A B C D E F G H I J K L M N O;
    A B C D E F G H I J K L M N O P;
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn from_js_multi_len() {
        assert_eq!(<((),)>::LEN, 1);
        assert_eq!(<((), ())>::LEN, 2);
        assert_eq!(<((), (), ())>::LEN, 3);
    }

    #[test]
    fn call_js_fn_with_var_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (...x) => [x.length, ...x]
                "#,
                )
                .unwrap();
            func.call(RestValues::from(vec![1, 2, 3])).unwrap()
        });
        assert_eq!(res.len(), 4);
        assert_eq!(res[0], 3);
        assert_eq!(res[1], 1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 3);
    }

    #[test]
    fn call_js_fn_with_rest_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (a, b, ...x) => [a, b, x.length, ...x]
                "#,
                )
                .unwrap();
            func.call((-2, -1, RestValues::from(vec![1, 2]))).unwrap()
        });
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], -2);
        assert_eq!(res[1], -1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 1);
        assert_eq!(res[4], 2);
    }

    #[test]
    fn call_rust_fn_with_var_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func = Function::new(
                ctx,
                "test_fn",
                |_ctx, _this: Value, args: RestValues<i8>| -> Result<_> {
                    use std::iter::once;
                    Ok(once(args.len() as i8)
                        .chain(args.iter().cloned())
                        .collect::<Vec<_>>())
                },
            )
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  test_fn(1, 2, 3)
                "#,
            )
            .unwrap()
        });
        assert_eq!(res.len(), 4);
        assert_eq!(res[0], 3);
        assert_eq!(res[1], 1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 3);
    }

    #[test]
    fn call_rust_fn_with_rest_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func = Function::new(
                ctx,
                "test_fn",
                |_ctx, _this: Value, (arg1, arg2, args): (i8, i8, RestValues<i8>)| -> Result<_> {
                    use std::iter::once;
                    Ok(once(arg1)
                        .chain(once(arg2))
                        .chain(once(args.len() as i8))
                        .chain(args.iter().cloned())
                        .collect::<Vec<_>>())
                },
            )
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  test_fn(-2, -1, 1, 2)
                "#,
            )
            .unwrap()
        });
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], -2);
        assert_eq!(res[1], -1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 1);
        assert_eq!(res[4], 2);
    }
}
