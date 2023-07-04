//! Utilities for converting promises to futures and vice versa.

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};

use crate::{
    atom::PredefinedAtom, function::This, qjs, safe_ref::Ref, CatchResultExt, CaughtError,
    CaughtResult, Ctx, Exception, FromJs, Function, IntoJs, Object, Result, ThrowResultExt, Value,
};

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
    fn poll(&self, waker: Waker) -> Option<Waker> {
        self.waker.replace(Some(waker))
    }

    fn take_result(&self) -> Option<CaughtResult<'js, T>> {
        self.result.take()
    }

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

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext) -> Poll<Self::Output> {
        let ctx = self.promise.ctx();
        if let Some(x) = self.state.take_result() {
            return Poll::Ready(x.throw(ctx));
        }

        if self.state.poll(cx.waker().clone()).is_none() {
            let then: Function = self.promise.get(PredefinedAtom::Then)?;
            let state = self.state.clone();
            let resolve = Function::new(ctx, move |value: Value<'js>| {
                let ctx = value.ctx();
                let t = T::from_js(ctx, value).catch(ctx);
                state.resolve(t);
            });
            let state = self.state.clone();
            let reject = Function::new(ctx, move |value: Value<'js>| {
                let e =
                    if let Some(e) = value.clone().into_object().and_then(Exception::from_object) {
                        CaughtError::Exception(e)
                    } else {
                        CaughtError::Value(value)
                    };
                state.resolve(Err(e))
            });
            then.call((This(self.promise.clone()), resolve, reject))?;
        };
        Poll::Pending
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
    T: Future<Output = R> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (promise, resolve, reject) = ctx.promise()?;

        let future = async move {
            let err = match self.0.await.into_js(ctx).catch(ctx) {
                Ok(x) => resolve.call::<_, ()>((x,)),
                Err(e) => match e {
                    CaughtError::Exception(e) => reject.call::<_, ()>((e,)),
                    CaughtError::Value(e) => reject.call::<_, ()>((e,)),
                    CaughtError::Error(e) => {
                        let is_exception = unsafe { qjs::JS_IsException(e.throw(ctx)) };
                        debug_assert!(is_exception);
                        let e = ctx.catch();
                        reject.call::<_, ()>((e,))
                    }
                },
            };
            // TODO figure out something better to do here.
            if let Err(e) = err {
                println!("promise handle function returned error:{}", e);
            }
        };
        ctx.spawn(future);
        Ok(promise.into_value())
    }
}
