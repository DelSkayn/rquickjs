use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{Ctx, Function, IntoJs, Result, Value};

use super::IntoJsFunc;

/// Helper type to implement ToJsFunction for closure by constraining arguments.
pub struct Func<T, P>(T, PhantomData<P>);

impl<'js, T, P> Func<T, P>
where
    T: IntoJsFunc<'js, P>,
{
    pub fn new(t: T) -> Self {
        Func(t, PhantomData)
    }
}

impl<'js, T, P> From<T> for Func<T, P>
where
    T: IntoJsFunc<'js, P>,
{
    fn from(value: T) -> Self {
        Func(value, PhantomData)
    }
}

impl<'js, T, P> IntoJs<'js> for Func<T, P>
where
    T: IntoJsFunc<'js, P> + 'js,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let function = Function::new(ctx.clone(), self.0);
        function.into_js(ctx)
    }
}

/// helper type for working setting and retrieving `this` values.
pub struct This<T>(pub T);

/// helper type for retrieving function object on which a function is called..
pub struct FuncArg<T>(pub T);

/// Helper type for optional paramaters.
pub struct Opt<T>(pub Option<T>);

/// Helper type for rest and spread arguments.
pub struct Rest<T>(pub Vec<T>);

/// Helper type for converting an option into null instead of undefined.
pub struct Null<T>(pub Option<T>);

/// A type to flatten tuples into another tuple.
///
/// ToArgs is only implemented for tuples with a length of up to 8.
/// If you need more arguments you can use this type to extend arguments with upto 8 additional
/// arguments recursivily.
pub struct Flat<T>(pub T);

/// Helper type for making an parameter set exhaustive.
pub struct Exhaustive;

/// Helper type for creating a function from a closure which returns a future.
pub struct Async<T>(pub T);

/// Helper type for creating a function from a closure which implements FnMut
///
/// When called will try to borrow the internal [`RefCell`], if this is not
/// possible it will return an error.
pub struct MutFn<T>(pub RefCell<T>);

impl<T> MutFn<T> {
    pub fn new(t: T) -> Self {
        MutFn(RefCell::new(t))
    }
}

impl<T> From<T> for MutFn<T> {
    fn from(value: T) -> Self {
        MutFn::new(value)
    }
}

/// Helper type for creating a function from a closure which implements FnMut
///
/// When called, will take the internal value leaving it empty. If the internal
/// value was already empty it will return a error.
pub struct OnceFn<T>(pub Cell<Option<T>>);

impl<T> OnceFn<T> {
    pub fn new(t: T) -> Self {
        Self(Cell::new(Some(t)))
    }
}

impl<T> From<T> for OnceFn<T> {
    fn from(value: T) -> Self {
        OnceFn::new(value)
    }
}

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

type_impls! {
    MutFn<F>(RefCell<F>): AsRef Deref;
    OnceFn<F>(Cell<Option<F>>): AsRef Deref;
    This<T>(T): into_inner From AsRef AsMut Deref DerefMut;
    FuncArg<T>(T): into_inner From AsRef AsMut Deref DerefMut;
    Opt<T>(Option<T>): into_inner From AsRef AsMut Deref DerefMut;
    Rest<T>(Vec<T>): into_inner From AsRef AsMut Deref DerefMut;
    Flat<T>(T): into_inner From AsRef AsMut Deref DerefMut;
}
