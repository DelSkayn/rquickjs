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
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
            let resolve = Function::new(ctx.clone(), move |ctx: Ctx<'js>, value: Value<'js>| {
                let t = T::from_js(&ctx, value).catch(&ctx);
                state.resolve(t);
            });
            let state = self.state.clone();
            let reject = Function::new(ctx.clone(), move |value: Value<'js>| {
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
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let (promise, resolve, reject) = ctx.promise()?;
        let ctx_clone = ctx.clone();

        let future = async move {
            let err = match self.0.await.into_js(&ctx_clone).catch(&ctx_clone) {
                Ok(x) => resolve.call::<_, ()>((x,)),
                Err(e) => match e {
                    CaughtError::Exception(e) => reject.call::<_, ()>((e,)),
                    CaughtError::Value(e) => reject.call::<_, ()>((e,)),
                    CaughtError::Error(e) => {
                        let is_exception = unsafe { qjs::JS_IsException(e.throw(&ctx_clone)) };
                        debug_assert!(is_exception);
                        let e = ctx_clone.catch();
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

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    use crate::{
        async_with,
        function::{Async, Func},
        AsyncContext, AsyncRuntime, CaughtError, Exception, Function, Result,
    };

    async fn set_timeout<'js>(cb: Function<'js>, number: f64) -> Result<()> {
        tokio::time::sleep(Duration::from_secs_f64(number / 1000.0)).await;
        cb.call::<_, ()>(())
    }

    #[tokio::test]
    async fn promise() {
        let rt = AsyncRuntime::new().unwrap();
        let ctx = AsyncContext::full(&rt).await.unwrap();

        async_with!(ctx => |ctx| {
            ctx.globals().set("setTimeout",Func::from(Async(set_timeout))).unwrap();

            let func = ctx
                .eval::<Function, _>(
                    r"
                    (function(){
                        return new Promise((resolve) => {
                            setTimeout(x => {
                                resolve(42)
                            },100)
                        })
                    })
                    ",
                )
                .catch(&ctx)
                .unwrap();
            let promise: Promise<i32> = func.call(()).unwrap();
            assert_eq!(promise.await.catch(&ctx).unwrap(), 42);

            let func = ctx
                .eval::<Function, _>(
                    r"
                    (function(){
                        return new Promise((_,reject) => {
                            setTimeout(x => {
                                reject(42)
                            },100)
                        })
                    })
                    ",
                )
                .catch(&ctx)
                .unwrap();
            let promise: Promise<()> = func.call(()).unwrap();
            let err = promise.await.catch(&ctx);
            match err {
                Err(CaughtError::Value(v)) => {
                    assert_eq!(v.as_int().unwrap(), 42)
                }
                _ => panic!(),
            }
        })
        .await
    }

    #[tokio::test]
    async fn promised() {
        let rt = AsyncRuntime::new().unwrap();
        let ctx = AsyncContext::full(&rt).await.unwrap();

        async_with!(ctx => |ctx| {
            let promised = Promised::from(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                42
            });

            let function = ctx.eval::<Function,_>(r"
                (async function(v){
                    let val = await v;
                    if(val !== 42){
                        throw new Error('not correct value')
                    }
                })
            ").catch(&ctx).unwrap();

            function.call::<_,Promise<()>>((promised,)).unwrap().await.unwrap();

            let ctx_clone = ctx.clone();
            let promised = Promised::from(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Result::<()>::Err(Exception::throw_message(&ctx_clone, "some_message"))
            });

            let function = ctx.eval::<Function,_>(r"
                (async function(v){
                    try{
                        await v;
                    }catch(e) {
                        if (e.message !== 'some_message'){
                            throw new Error('wrong error')
                        }
                        return
                    }
                    throw new Error('no error thrown')
                })
            ")
                .catch(&ctx)
                .unwrap();


            function.call::<_,Promise<()>>((promised,)).unwrap().await.unwrap()
        })
        .await
    }
}
