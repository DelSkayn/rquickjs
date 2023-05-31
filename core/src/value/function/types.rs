use crate::{function::AsFunction, markers::ParallelSend, Ctx, Function, IntoJs, Result, Value};
use std::{
    cell::RefCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// The wrapper for method functions
///
/// The method-like functions is functions which get `this` as the first argument. This wrapper allows receive `this` directly as first argument and do not requires using [`This`] for that purpose.
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, Function, function::{Method, This}};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// let func = Function::new(ctx, Method(|this: i32, factor: i32| {
///     this * factor
/// }))?;
/// assert_eq!(func.call::<_, i32>((This(3), 2))?, 6);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[repr(transparent)]
pub struct Method<F>(pub F);

/// The wrapper for function to convert is into JS
///
/// The Rust functions should be wrapped to convert it to JS using [`IntoJs`] trait.
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, function::Func};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// // Anonymous function
/// ctx.globals().set("sum", Func::from(|a: i32, b: i32| a + b))?;
/// assert_eq!(ctx.eval::<i32, _>("sum(3, 2)")?, 5);
/// assert_eq!(ctx.eval::<usize, _>("sum.length")?, 2);
/// assert_eq!(ctx.eval::<String, _>("sum.name")?, "");
/// // Call/apply works as expected
/// assert_eq!(ctx.eval::<i32, _>("sum.call(sum, 3, 2)")?, 5);
/// assert_eq!(ctx.eval::<i32, _>("sum.apply(sum, [3, 2])")?, 5);
///
/// // Named function
/// ctx.globals().set("prod", Func::new("multiply", |a: i32, b: i32| a * b))?;
/// assert_eq!(ctx.eval::<i32, _>("prod(3, 2)")?, 6);
/// assert_eq!(ctx.eval::<usize, _>("prod.length")?, 2);
/// assert_eq!(ctx.eval::<String, _>("prod.name")?, "multiply");
/// // Call/apply works as expected
/// assert_eq!(ctx.eval::<i32, _>("prod.call(prod, 3, 2)")?, 6);
/// assert_eq!(ctx.eval::<i32, _>("prod.apply(prod, [3, 2])")?, 6);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[repr(transparent)]
pub struct Func<F>(pub F);

/// The wrapper for async functons
///
/// This type wraps returned future into [`Promised`](crate::Promised)
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, Function, function::Async};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// // Inorder to conver to a javascript promise the future must return a `Result`.
/// async fn my_func() -> Result<()> { Ok(()) }
/// let func = Function::new(ctx, Async(my_func));
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
pub struct Async<F>(pub F);

/// The wrapper for mutable functions
///
/// This wrapper is useful for closures which encloses mutable state.
#[repr(transparent)]
pub struct MutFn<F>(RefCell<F>);

impl<F> From<F> for MutFn<F> {
    fn from(func: F) -> Self {
        Self(RefCell::new(func))
    }
}

/// The wrapper for once functions
///
/// This wrapper is useful for callbacks which can be invoked only once.
#[repr(transparent)]
pub struct OnceFn<F>(RefCell<Option<F>>);

impl<F> From<F> for OnceFn<F> {
    fn from(func: F) -> Self {
        Self(RefCell::new(Some(func)))
    }
}

/// The wrapper to get `this` from input
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, function::This, Function};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// // Get the `this` value via arguments
/// let func = Function::new(ctx, |this: This<i32>, factor: i32| {
///     this.into_inner() * factor
/// })?;
/// // Pass the `this` value to a function
/// assert_eq!(func.call::<_, i32>((This(3), 2))?, 6);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct This<T>(pub T);

/// The wrapper to get optional argument from input
///
/// The [`Option`] type cannot be used for that purpose because it implements [`FromJs`](crate::FromJs) trait and requires the argument which may be `undefined`.
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, function::Opt, Function};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// let func = Function::new(ctx, |required: i32, optional: Opt<i32>| {
///     required * optional.into_inner().unwrap_or(1)
/// })?;
/// assert_eq!(func.call::<_, i32>((3,))?, 3);
/// assert_eq!(func.call::<_, i32>((3, 1))?, 3);
/// assert_eq!(func.call::<_, i32>((3, 2))?, 6);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct Opt<T>(pub Option<T>);

/// The wrapper the rest arguments from input
///
/// The [`Vec`] type cannot be used for that purpose because it implements [`FromJs`](crate::FromJs) and already used to convert JS arrays.
///
/// ```
/// # use rquickjs::{Runtime, Context, Result, function::Rest, Function};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// #
/// let func = Function::new(ctx, |required: i32, optional: Rest<i32>| {
///     optional.into_inner().into_iter().fold(required, |prod, arg| prod * arg)
/// })?;
/// assert_eq!(func.call::<_, i32>((3,))?, 3);
/// assert_eq!(func.call::<_, i32>((3, 2))?, 6);
/// assert_eq!(func.call::<_, i32>((3, 2, 1))?, 6);
/// assert_eq!(func.call::<_, i32>((3, 2, 1, 4))?, 24);
/// #
/// # Ok(())
/// # }).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Rest<T>(pub Vec<T>);

macro_rules! type_impls {
	  ($($type:ident <$($params:ident),*>($($fields:tt)*): $($impls:ident)*;)*) => {
        $(type_impls!{@impls $type [$($params)*]($($fields)*) $($impls)*})*
	  };

    (@impls $type:ident[$($params:ident)*]($($fields:tt)*) $impl:ident $($impls:ident)*) => {
        type_impls!{@impl $impl($($fields)*) $type $($params)*}
        type_impls!{@impls $type[$($params)*]($($fields)*) $($impls)*}
    };

    (@impls $type:ident[$($params:ident)*]($($fields:tt)*)) => {};

    (@impl into_inner($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> $type<$param $(, $params)*> {
            pub fn into_inner(self) -> $field {
                self.0
            }
        }
    };

    (@impl Into($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> From<$type<$param $(, $params)*>> for $field {
            fn from(this: $type<$param $(, $params)*>) -> Self {
                this.0
            }
        }
    };

    (@impl From($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> From<$field> for $type<$param $(, $params)*> {
            fn from(value: $field) -> Self {
                Self(value $(, type_impls!(@def $fields))*)
            }
        }
    };

    (@impl AsRef($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> AsRef<$field> for $type<$param $(, $params)*> {
            fn as_ref(&self) -> &$field {
                &self.0
            }
        }
    };

    (@impl AsMut($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> AsMut<$field> for $type<$param $(, $params)*> {
            fn as_mut(&mut self) -> &mut $field {
                &mut self.0
            }
        }
    };

    (@impl Deref($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> Deref for $type<$param $(, $params)*> {
            type Target = $field;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };

    (@impl DerefMut($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> DerefMut for $type<$param $(, $params)*> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };

    (@def $($t:tt)*) => { Default::default() };
}

impl<F, A, R> From<F> for Func<(F, PhantomData<(A, R)>)> {
    fn from(func: F) -> Self {
        Self((func, PhantomData))
    }
}

impl<'js, F, A, R> IntoJs<'js> for Func<(F, PhantomData<(A, R)>)>
where
    F: AsFunction<'js, A, R> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let data = self.0;
        Function::new(ctx, data.0)?.into_js(ctx)
    }
}

impl<N, F, A, R> Func<(N, F, PhantomData<(A, R)>)> {
    pub fn new(name: N, func: F) -> Self {
        Self((name, func, PhantomData))
    }
}

impl<'js, N, F, A, R> IntoJs<'js> for Func<(N, F, PhantomData<(A, R)>)>
where
    N: AsRef<str>,
    F: AsFunction<'js, A, R> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let data = self.0;
        let func = Function::new(ctx, data.1)?;
        func.set_name(data.0)?;
        func.into_js(ctx)
    }
}

type_impls! {
    Func<F>(F): AsRef Deref;
    MutFn<F>(RefCell<F>): AsRef Deref;
    OnceFn<F>(RefCell<Option<F>>): AsRef Deref;
    Method<F>(F): AsRef Deref;
    This<T>(T): into_inner From AsRef AsMut Deref DerefMut;
    Opt<T>(Option<T>): into_inner Into From AsRef AsMut Deref DerefMut;
    Rest<T>(Vec<T>): into_inner Into From AsRef AsMut Deref DerefMut;
}

#[cfg(feature = "futures")]
type_impls! {
    Async<F>(F): AsRef Deref;
}

impl<T> Rest<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}
