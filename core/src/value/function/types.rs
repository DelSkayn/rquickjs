use std::cell::{Cell, RefCell};

use crate::{class::Trace, markers::Invariant};

/// helper type for working setting and retrieving `this` values.
pub struct This<T>(pub T);

/// helper type for retrieving function object on which a function is called..
pub struct Func<T>(pub T);

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
#[cfg(feature = "futures")]
pub struct Async<T>(pub T);
