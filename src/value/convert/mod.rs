use crate::{context::Ctx, Atom, Result, Value};
mod atom;
mod from;
mod into;

/// For converting javascript values to rust values
///
/// This trait automaticly converts any value which can be
/// represented as an object, like [`Array`](struct.Array.html) to one if it is required.
pub trait FromJs<'js>: Sized {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>;
}

/// Trait for converting values from atoms.
pub trait FromAtom<'js>: Sized {
    fn from_atom(atom: Atom<'js>) -> Result<Self>;
}

/// For converting rust values to javascript values
pub trait IntoJs<'js> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// Trait for converting values to atoms.
pub trait IntoAtom<'js> {
    fn into_atom(self, ctx: Ctx<'js>) -> Atom<'js>;
}
