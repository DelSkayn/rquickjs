use crate::{context::Ctx, Atom, MultiValue, MultiValueJs, RestValues, Result, Value};
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

    const LEN: i32;
}

/// Trait for converting values from atoms.
pub trait FromAtom<'js>: Sized {
    fn from_atom(atom: Atom<'js>) -> Result<Self>;
}

/// For converting rust values to javascript values
pub trait IntoJs<'js> {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// For converting multiple of value to javascript
/// Mostly used for converting the arguments of a function from rust to javascript
pub trait IntoJsMulti<'js> {
    fn to_js_multi(self, ctx: Ctx<'js>) -> Result<MultiValueJs<'js>>;
}

/// Trait for converting values to atoms.
pub trait IntoAtom<'js> {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js>;
}
