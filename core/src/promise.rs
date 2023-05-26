use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};

use crate::{
    safe_ref::Ref, Ctx, Error, FromJs, Func, Function, IntoJs, Object, Result, This, Value,
};

pub struct InnerPromise<'js, T> {
    result: Cell<Option<Result<T>>>,
    waker: Cell<Option<Waker>>,
    promise: Object<'js>,
}

impl<'js, T> InnerPromise<'js, T> {
    pub fn resolve(&self, res: Result<T>) {
        self.result.set(Some(res));
        self.waker
            .take()
            .expect("resolver functions called twice")
            .wake()
    }
}

// RC here should always be safe.
// Promise has a 'js lifetime so it can't escape outside of the lock.
/// A promise which can be awaited as a future.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Promise<'js, T>(Ref<InnerPromise<'js, T>>);

impl<'js, T> Clone for Promise<'js, T> {
    fn clone(&self) -> Self {
        Promise(self.0.clone())
    }
}

// TODO validate.
unsafe impl<'js, T> Send for Promise<'js, T> {}
unsafe impl<'js, T> Sync for Promise<'js, T> {}

impl<'js, T> FromJs<'js> for Promise<'js, T>
where
    T: FromJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let promise = Object::from_js(ctx, value)?;
        Ok(Promise(Ref::new(InnerPromise {
            result: Cell::new(None),
            waker: Cell::new(None),
            promise,
        })))
    }
}

impl<'js, T> Future for Promise<'js, T>
where
    T: FromJs<'js> + 'js,
{
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext) -> Poll<Self::Output> {
        match self.0.result.take() {
            Some(x) => Poll::Ready(x),
            None => {
                if self.0.waker.replace(Some(cx.waker().clone())).is_none() {
                    // Waker is none so this is the first poll.
                    // Create reject and resolve callbacks
                    let c_prom = (*self).clone();
                    let resolve = Func::new("resolve", move |ctx: Ctx<'js>, value: Value<'js>| {
                        c_prom.0.resolve(T::from_js(ctx, value))
                    });
                    let c_prom = (*self).clone();
                    let reject = Func::new("reject", move |ctx: Ctx<'js>, value: Value<'js>| {
                        let (Ok(err) | Err(err)) = Error::from_js(ctx, value);
                        c_prom.0.resolve(Err(err));
                    });
                    // retrieve then
                    let then: Function = match self.0.promise.get("then") {
                        Err(e) => return Poll::Ready(Err(e)),
                        Ok(x) => x,
                    };
                    let prom_obj = self.0.promise.clone();
                    // register callbacks.
                    if let Err(e) = then.call::<_, ()>((This(prom_obj), resolve, reject)) {
                        return Poll::Ready(Err(e));
                    }
                }
                Poll::Pending
            }
        }
    }
}

/// Wrapper for futures to convert to JS promises
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
pub struct Promised<T>(pub T);

impl<'js, T, R> IntoJs<'js> for Promised<T>
where
    T: Future<Output = Result<R>> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (promise, resolve, reject) = ctx.promise()?;

        let future = async move {
            let err = match self.0.await.and_then(|v| v.into_js(ctx)) {
                Ok(x) => resolve.call::<_, ()>((x,)),
                Err(e) => {
                    let v = e
                        .into_js(ctx)
                        .expect("recieved eror while trying to report error");
                    reject.call::<_, ()>((v,))
                }
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
