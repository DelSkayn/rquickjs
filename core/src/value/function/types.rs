use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
};

use super::ToJsFunction;

/// Helper type to implement ToJsFunction for closure by constraining arguments.
pub struct Func<T, P>(T, PhantomData<P>);

impl<'js, T, P> From<T> for Func<T, P>
where
    T: ToJsFunction<'js, P>,
{
    fn from(value: T) -> Self {
        Func(value, PhantomData)
    }
}

/// helper type for working setting and retrieving `this` values.
pub struct This<T>(pub T);

/// helper type for retrieving function object on which a function is called..
pub struct ThisFunc<T>(pub T);

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
/// When called through [`CellFn`] will try to borrow the internal [`RefCell`], if this is not
/// possible it will return an error.
pub struct Mut<T>(pub RefCell<T>);

/// Helper type for creating a function from a closure which implements FnMut
///
/// When called through [`CellFn`] will take the internal value leaving it empty. If the internal
/// value was already empty it will return a error.
pub struct Once<T>(pub Cell<Option<T>>);
