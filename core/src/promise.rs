//! Utilities for converting promises to futures and vice versa.

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};

use crate::{safe_ref::Ref, CaughtResult, Ctx, FromJs, IntoJs, Object, Result, Value};

/// Future-aware promise
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Promise<'js, T> {
    state: Ref<State<'js, T>>,
    promise: Object<'js>,
}

struct State<'js, T> {
    waker: Cell<Option<Waker>>,
    result: Cell<Option<CaughtResult<'js, T>>>,
}

impl<'js, T: 'js> State<'js, T> {
    fn resolve(&self, result: CaughtResult<'js, T>) {
        self.result.set(Some(result));
        self.waker
            .take()
            .expect("promise resolved before being polled")
            .wake();
    }
}

unsafe impl<'js, T> Send for State<'js, T> {}
unsafe impl<'js, T> Sync for State<'js, T> {}

impl<'js, T> FromJs<'js> for Promise<'js, T>
where
    T: FromJs<'js> + 'js,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let promise = Object::from_js(ctx, value)?;
        let state = Ref::new(State {
            waker: Cell::new(None),
            result: Cell::new(None),
        });

        Ok(Promise { state, promise })
    }
}

impl<'js, T> Future for Promise<'js, T>
where
    T: FromJs<'js> + 'js,
{
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext) -> Poll<Self::Output> {
        todo!()
    }
}

/// Wrapper for futures to convert to JS promises
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
pub struct Promised<T>(pub T);

impl<T> From<T> for Promised<T> {
    fn from(future: T) -> Self {
        Self(future)
    }
}

impl<'js, T, R> IntoJs<'js> for Promised<T>
where
    T: Future<Output = Result<R>> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        todo!()
    }
}
