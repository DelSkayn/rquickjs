use crate::{AsFunction, Ctx, Function, IntoJs, Result, SendWhenParallel, Value};
use std::{
    cell::RefCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// The wrapper for method functions
#[repr(transparent)]
pub struct Method<F>(pub F);

/// The wrapper for function to convert into JS
#[repr(transparent)]
pub struct Func<F>(pub F);

/// The wrapper for mutable functions
#[repr(transparent)]
pub struct MutFn<F>(RefCell<F>);

impl<F> From<F> for MutFn<F> {
    fn from(func: F) -> Self {
        Self(RefCell::new(func))
    }
}

/// The wrapper for once functions
#[repr(transparent)]
pub struct OnceFn<F>(RefCell<Option<F>>);

impl<F> From<F> for OnceFn<F> {
    fn from(func: F) -> Self {
        Self(RefCell::new(Some(func)))
    }
}

/// The wrapper to get `this` from input
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct This<T>(pub T);

/// The wrapper to get optional argument from input
///
/// Which is needed because the `Option` implements `FromJs` so requires the argument which may be `undefined`.
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct Opt<T>(pub Option<T>);

/// The wrapper the rest arguments from input
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

    (@impl into($field:ty $(, $fields:tt)*) $type:ident $param:ident $($params:ident)*) => {
        impl<$param $(, $params)*> $type<$param $(, $params)*> {
            pub fn into(self) -> $field {
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
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
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
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
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
    This<T>(T): into From AsRef AsMut Deref DerefMut;
    Opt<T>(Option<T>): into From AsRef AsMut Deref DerefMut;
    Rest<T>(Vec<T>): Into From AsRef AsMut Deref DerefMut;
}

impl<T> Rest<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}
