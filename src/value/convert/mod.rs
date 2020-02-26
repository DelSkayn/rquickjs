use crate::{context::Ctx, Result, Value};
mod from;
mod multi;
mod to;

/// For converting javascript values to rust values
pub trait FromJs<'js>: Sized {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>;
}

/// For converting rust values to javascript values
pub trait ToJs<'js> {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// For converting multiple of value to javascript
pub trait ToJsMulti<'js> {
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<Vec<Value>>;
}
