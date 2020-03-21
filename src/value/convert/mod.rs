use crate::{context::Ctx, Atom, MultiValue, Result, Value};
mod atom;
mod from;
mod multi;
mod to;

/// For converting javascript values to rust values
///
/// This trait automaticly converts any value which can be
/// represented as an object, like [`Array`](struct.Array.html) to one if it is required.
pub trait FromJs<'js>: Sized {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>;
}

/// For converting multiple of value to javascript
pub trait FromJsMulti<'js>: Sized {
    fn from_js_multi(ctx: Ctx<'js>, value: MultiValue<'js>) -> Result<Self>;

    fn len() -> i64;
}

/// For converting rust values to javascript values
pub trait ToJs<'js> {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// For converting multiple of value to javascript
/// Mostly used for converting the arguments of a function from rust to javascript
pub trait ToJsMulti<'js> {
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<Vec<Value>>;
}

/// Trait for converting values to atoms.
pub trait ToAtom<'js> {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js>;
}
