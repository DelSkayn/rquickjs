use std::iter::Iterator as StdIterator;

use super::IntoJsIter;
use crate::{Ctx, IntoJs, Result};

impl<'js, T, I> IntoJsIter<'js> for T
where
    T: StdIterator<Item = I>,
    I: IntoJs<'js>,
{
    type Item = I;

    fn next(&mut self, _ctx: Ctx<'_>, _position: usize) -> Result<Option<I>> {
        Ok(self.next())
    }
}

/// Helper type for creating an iterator from a closure which implements [`Fn`]
pub struct IterFn<F>(F);

impl<F> IterFn<F> {
    pub fn new(f: F) -> Self {
        IterFn(f)
    }
}

impl<F> From<F> for IterFn<F> {
    fn from(value: F) -> Self {
        IterFn::new(value)
    }
}

impl<'js, F, I> IntoJsIter<'js> for IterFn<F>
where
    F: Fn(Ctx<'js>, usize) -> Result<Option<I>>,
    I: IntoJs<'js>,
{
    type Item = I;

    fn next(&mut self, ctx: Ctx<'js>, position: usize) -> Result<Option<I>> {
        self.0(ctx, position)
    }
}

/// Helper type for creating an iterator from a closure which implements [`FnMut`]
pub struct IterFnMut<F>(F);

impl<F> IterFnMut<F> {
    pub fn new(f: F) -> Self {
        IterFnMut(f)
    }
}

impl<F> From<F> for IterFnMut<F> {
    fn from(value: F) -> Self {
        IterFnMut::new(value)
    }
}

impl<'js, F, I> IntoJsIter<'js> for IterFnMut<F>
where
    F: FnMut(Ctx<'js>, usize) -> Result<Option<I>>,
    I: IntoJs<'js>,
{
    type Item = I;

    fn next(&mut self, ctx: Ctx<'js>, position: usize) -> Result<Option<I>> {
        self.0(ctx, position)
    }
}
