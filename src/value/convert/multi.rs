use super::{ToJs, ToJsMulti};
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
