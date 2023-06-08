//! Utility types and traits.

use std::marker::PhantomData;

// Super nice trick taken from the rlua library.
// Can be used to pin a lifetime so that all functions which use
// that lifetime can only us that single lifetime and not one which
// is variant over that lifetime
/// A marker struct which marks a lifetime as invariant.
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Invariant<'inv>(PhantomData<&'inv mut &'inv fn(&'inv ()) -> &'inv ()>);

impl<'inv> Invariant<'inv> {
    pub fn new() -> Self {
        Invariant(PhantomData)
    }

    pub fn new_ref<T>(_v: &'inv T) -> Self {
        Invariant(PhantomData)
    }
}

/// The marker trait which requires [`Send`] when `"parallel"` feature is used
#[cfg(not(feature = "parallel"))]
pub trait ParallelSend {}

#[cfg(feature = "parallel")]
pub trait ParallelSend: Send {}

#[cfg(not(feature = "parallel"))]
impl<T> ParallelSend for T {}

#[cfg(feature = "parallel")]
impl<T: Send> ParallelSend for T {}
