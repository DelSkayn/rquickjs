use super::{ArgsValue, ArgsValueJs, FromJs, FromJsArgs, IntoJs, IntoJsArgs, RestValues, Value};
use crate::{Ctx, Result};
use std::iter::{empty, once};

impl<'js> IntoJsArgs<'js> for ArgsValueJs<'js> {
    fn into_js_args(self, _: Ctx<'js>) -> Result<ArgsValueJs<'js>> {
        Ok(self)
    }
}

impl<'js, T: IntoJs<'js>> IntoJsArgs<'js> for T {
    fn into_js_args(self, ctx: Ctx<'js>) -> Result<ArgsValueJs<'js>> {
        Ok(vec![self.into_js(ctx)?].into())
    }
}

impl<'js> FromJsArgs<'js> for ArgsValue<'js> {
    fn from_js_args(_: Ctx<'js>, value: ArgsValue<'js>) -> Result<Self> {
        Ok(value)
    }

    const LEN: i32 = -1;
}

impl<'js> FromJsArgs<'js> for () {
    fn from_js_args(_: Ctx<'js>, _: ArgsValue<'js>) -> Result<Self> {
        Ok(())
    }

    const LEN: i32 = 0;
}

impl<'js, X> IntoJsArgs<'js> for RestValues<X>
where
    X: IntoJs<'js>,
{
    fn into_js_args(self, ctx: Ctx<'js>) -> Result<ArgsValueJs<'js>> {
        let rest: Vec<_> = self.into();
        let iter = rest.into_iter().map(|value| value.into_js(ctx));
        Ok(iter.collect::<Result<Vec<_>>>()?.into())
    }
}

impl<'js, X> FromJsArgs<'js> for RestValues<X>
where
    X: FromJs<'js>,
{
    fn from_js_args(ctx: Ctx<'js>, mut value: ArgsValue<'js>) -> Result<Self> {
        Ok(value
            .iter()
            .map(|value| X::from_js(ctx, value))
            .collect::<Result<Vec<_>>>()?
            .into())
    }

    const LEN: i32 = 0;
}

macro_rules! impl_from_to_js_args {
    ($($($t:ident)*;)*) => {
        $(
            impl<'js, $($t,)*> IntoJsArgs<'js> for ($($t,)*)
            where
                $($t: IntoJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn into_js_args(self, ctx: Ctx<'js>) -> Result<ArgsValueJs<'js>>{
                    let ($($t,)*) = self;
                    Ok(vec![
                        $($t.into_js(ctx)?,)*
                    ].into())
                }
            }

            impl<'js, $($t,)* X> IntoJsArgs<'js> for ($($t,)* RestValues<X>)
            where
                $($t: IntoJs<'js>,)*
                X: IntoJs<'js>,
            {
                #[allow(non_snake_case)]
                fn into_js_args(self, ctx: Ctx<'js>) -> Result<ArgsValueJs<'js>>{
                    let ($($t,)* X) = self;
                    let iter = empty();
                    $(let iter = iter.chain(once($t.into_js(ctx))));*;
                    let rest: Vec<_> = X.into();
                    let iter = iter.chain(rest.into_iter().map(|value| value.into_js(ctx)));
                    Ok(iter.collect::<Result<Vec<_>>>()?.into())
                }
            }

            impl<'js, $($t,)*> FromJsArgs<'js> for ($($t,)*)
            where
                $($t: FromJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn from_js_args(ctx: Ctx<'js>, mut value: ArgsValue<'js>) -> Result<Self> {
                    let mut iter = value.iter();
                    Ok((
                        $({
                            let v = iter.next()
                                .unwrap_or(Value::Undefined);
                            $t::from_js(ctx,v)?
                        },)*
                    ))
                }

                const LEN: i32 = impl_from_to_js_args!(@count $($t),*);
            }

            impl<'js, $($t,)* X> FromJsArgs<'js> for ($($t,)* RestValues<X>)
            where
                $($t: FromJs<'js>,)*
                X: FromJs<'js>,
            {
                #[allow(non_snake_case)]
                fn from_js_args(ctx: Ctx<'js>, mut value: ArgsValue<'js>) -> Result<Self> {
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

                const LEN: i32 = impl_from_to_js_args!(@count $($t),*);
            }
        )*
    };

    (@count $($t:ident),*) => {
        0 $(+ impl_from_to_js_args!(@1 $t))*
    };

    (@1 $($t:tt)*) => { 1 };
}

impl_from_to_js_args! {
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
    fn from_js_args_len() {
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
