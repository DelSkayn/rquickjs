use crate::{Atom, Ctx, Result, Value};

mod atom;
mod coerce;
mod from;
mod into;

/// The wrapper for values to force coercion
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Coerced<T>(pub T);

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

/// The `FromIterator` trait to use with `Ctx`
pub trait FromIteratorJs<'js, A>: Sized {
    type Item;

    fn from_iter_js<T>(ctx: Ctx<'js>, iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = A>;
}

/// For converting rust values to javascript values
pub trait IntoJs<'js> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>>;
}

/// Trait for converting values to atoms.
pub trait IntoAtom<'js> {
    fn into_atom(self, ctx: Ctx<'js>) -> Atom<'js>;
}

/// The `Iterator` trait extension to support `Ctx`
pub trait IteratorJs<'js, A> {
    fn collect_js<B>(self, ctx: Ctx<'js>) -> Result<B>
    where
        B: FromIteratorJs<'js, A>;
}

impl<'js, T, A> IteratorJs<'js, A> for T
where
    T: Iterator<Item = A>,
{
    fn collect_js<B>(self, ctx: Ctx<'js>) -> Result<B>
    where
        B: FromIteratorJs<'js, A>,
    {
        B::from_iter_js(ctx, self)
    }
}
