use crate::{context::Ctx, ArgsValue, ArgsValueJs, Atom, RestValues, Result, Value};
mod atom;
mod from;
mod into;
mod multi;

/// For converting javascript values to rust values
///
/// This trait automaticly converts any value which can be
/// represented as an object, like [`Array`](struct.Array.html) to one if it is required.
pub trait FromJs<'js>: Sized {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>;
}

/// For converting multiple of value to javascript
pub trait FromJsArgs<'js>: Sized {
    fn from_js_args(ctx: Ctx<'js>, value: ArgsValue<'js>) -> Result<Self>;

    const LEN: i32;
}

/// Trait for converting values from atoms.
pub trait FromAtom<'js>: Sized {
    fn from_atom(atom: Atom<'js>) -> Result<Self>;
}

/// For converting rust values to javascript values
pub trait IntoJs<'js> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// For converting multiple of value to javascript
/// Mostly used for converting the arguments of a function from rust to javascript
pub trait IntoJsArgs<'js> {
    fn into_js_args(self, ctx: Ctx<'js>) -> Result<ArgsValueJs<'js>>;
}

/// Trait for converting values to atoms.
pub trait IntoAtom<'js> {
    fn into_atom(self, ctx: Ctx<'js>) -> Atom<'js>;
}
