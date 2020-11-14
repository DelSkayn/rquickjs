use super::{FromJs, FromJsMulti, MultiValue, ToJs, ToJsMulti};
use crate::{Ctx, Result, Value};

impl<'js> ToJsMulti<'js> for Vec<Value<'js>> {
    fn to_js_multi(self, _: Ctx<'js>) -> Result<Vec<Value<'js>>> {
        Ok(self)
    }
}

impl<'js, T: ToJs<'js>> ToJsMulti<'js> for T {
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<Vec<Value<'js>>> {
        Ok(vec![self.to_js(ctx)?])
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

macro_rules! impl_from_to_js_multi {
    ($($($t:ident)*;)*) => {
        $(
            impl<'js, $($t,)*> ToJsMulti<'js> for ($($t,)*)
            where $($t: ToJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn to_js_multi(self, ctx: Ctx<'js>) -> Result<Vec<Value<'js>>>{
                    let ($($t,)*) = self;
                    Ok(vec![
                        $($t.to_js(ctx)?,)*
                    ])
                }
            }

            impl<'js, $($t,)*> FromJsMulti<'js> for ($($t,)*)
            where $($t: FromJs<'js>,)*
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
    use super::*;

    #[test]
    fn from_js_multi_len() {
        assert_eq!(<((),)>::LEN, 1);
        assert_eq!(<((), ())>::LEN, 2);
        assert_eq!(<((), (), ())>::LEN, 3);
    }
}
