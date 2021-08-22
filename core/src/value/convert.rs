use crate::{Atom, Ctx, Result, Value};

mod atom;
mod coerce;
mod from;
mod into;

/// The wrapper for values to force coercion
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, Coerced};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// // Coercion to string
/// assert_eq!(ctx.eval::<Coerced<String>, _>("`abc`")?.0, "abc");
/// assert_eq!(ctx.eval::<Coerced<String>, _>("123")?.0, "123");
/// assert_eq!(ctx.eval::<Coerced<String>, _>("[1,'a']")?.0, "1,a");
/// assert_eq!(ctx.eval::<Coerced<String>, _>("({})")?.0, "[object Object]");
///
/// // Coercion to integer
/// assert!(ctx.eval::<i32, _>("123.5").is_err());
/// assert_eq!(ctx.eval::<Coerced<i32>, _>("123.5")?.0, 123);
///
/// assert!(ctx.eval::<i32, _>("`123`").is_err());
/// assert_eq!(ctx.eval::<Coerced<i32>, _>("`123`")?.0, 123);
///
/// // Coercion to floating-point
/// assert_eq!(ctx.eval::<f64, _>("123")?, 123.0);
/// assert_eq!(ctx.eval::<Coerced<f64>, _>("123")?.0, 123.0);
///
/// assert!(ctx.eval::<f64, _>("`123.5`").is_err());
/// assert_eq!(ctx.eval::<Coerced<f64>, _>("`123.5`")?.0, 123.5);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Coerced<T>(pub T);

/// For converting javascript values to rust values
///
/// This trait automatically converts any value which can be
/// represented as an object, like [`Array`](crate::Array)
/// to one if it is required.
pub trait FromJs<'js>: Sized {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>;
}

/// Trait for converting values from atoms.
pub trait FromAtom<'js>: Sized {
    fn from_atom(atom: Atom<'js>) -> Result<Self>;
}

/// The Rust's [`FromIterator`](std::iter::FromIterator) trait to use with [`Ctx`]
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

/// The Rust's [`Iterator`] trait extension which works with [`Ctx`]
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
