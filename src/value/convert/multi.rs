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

macro_rules! impl_to_js_multi{
    ($($t:ident),+) => {
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
    }
}

macro_rules! impl_from_js_multi{
    ($num:expr, $($t:ident),*) => {
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

            const LEN: i32 = $num;
        }
    }
}

impl_to_js_multi!(A);
impl_to_js_multi!(A, B);
impl_to_js_multi!(A, B, C);
impl_to_js_multi!(A, B, C, D);
impl_to_js_multi!(A, B, C, D, E);
impl_to_js_multi!(A, B, C, D, E, F);
impl_to_js_multi!(A, B, C, D, E, F, G);
impl_to_js_multi!(A, B, C, D, E, F, G, H);
impl_to_js_multi!(A, B, C, D, E, F, G, H, I);
impl_to_js_multi!(A, B, C, D, E, F, G, H, I, J);
impl_to_js_multi!(A, B, C, D, E, F, G, H, I, J, K);

impl_from_js_multi!(1, A);
impl_from_js_multi!(2, A, B);
impl_from_js_multi!(3, A, B, C);
impl_from_js_multi!(4, A, B, C, D);
impl_from_js_multi!(5, A, B, C, D, E);
impl_from_js_multi!(6, A, B, C, D, E, F);
impl_from_js_multi!(7, A, B, C, D, E, F, G);
impl_from_js_multi!(8, A, B, C, D, E, F, G, H);
impl_from_js_multi!(9, A, B, C, D, E, F, G, H, I);
impl_from_js_multi!(10, A, B, C, D, E, F, G, H, I, J);
impl_from_js_multi!(11, A, B, C, D, E, F, G, H, I, J, K);
