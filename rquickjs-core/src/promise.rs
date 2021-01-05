#[cfg(feature = "deferred-resolution")]
use crate::qjs;
#[cfg(any(feature = "tokio", feature = "async-std"))]
use crate::{Context, IntoJs, Persistent};
use crate::{
    Context, Ctx, Error, FromJs, Func, Function, IntoJs, Mut, Object, Persistent, Ref, Result,
    SendWhenParallel, This, Value,
};
#[cfg(any(feature = "tokio", feature = "async-std"))]
use std::mem;
use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};

/// Future-aware promise
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Promise<T> {
    state: Ref<Mut<State<T>>>,
}

struct State<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
}

impl<T> State<T> {
    fn resolve(&mut self, result: Result<T>) {
        self.result = Some(result);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl<T> Default for State<T> {
    fn default() -> Self {
        Self {
            result: None,
            waker: None,
        }
    }
}

impl<'js, T> FromJs<'js> for Promise<T>
where
    T: FromJs<'js> + SendWhenParallel + 'static,
{
    fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_value(value)?;
        let then: Function = obj.get("then")?;
        let state = Ref::new(Mut::new(State::default()));
        let on_ok = Func::new("onSuccess", {
            let state = state.clone();
            move |ctx: Ctx<'js>, value: Value<'js>| {
                let mut state = state.lock();
                state.resolve(T::from_js(ctx, value));
            }
        });
        let on_err = Func::new("onError", {
            let state = state.clone();
            move |error: Error| {
                let mut state = state.lock();
                state.resolve(Err(error));
            }
        });
        then.call((This(obj), on_ok, on_err))?;
        Ok(Self { state })
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Wrapper for futures to convert to JS promises
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
pub struct PromiseJs<T>(pub T);

impl<T> From<T> for PromiseJs<T> {
    fn from(future: T) -> Self {
        Self(future)
    }
}

#[cfg(any(feature = "async-std", feature = "tokio"))]
impl<'js, T> IntoJs<'js> for PromiseJs<T>
where
    T: Future + SendWhenParallel + 'static,
    for<'js_> T::Output: IntoJs<'js_> + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (promise, then, catch) = ctx.promise()?;

        let then = Persistent::save(ctx, then);
        let catch = Persistent::save(ctx, catch);

        let context = Context::from_ctx(ctx)?;
        let future = self.0;

        ctx.spawn_async(async move {
            let result = future.await;

            context.with(|ctx: Ctx| {
                match result.into_js(ctx) {
                    Ok(value) => {
                        mem::drop(catch);
                        resolve(ctx, then.restore(ctx).unwrap(), value)
                    }
                    Err(error) => {
                        mem::drop(then);
                        resolve(
                            ctx,
                            catch.restore(ctx).unwrap(),
                            error.into_js(ctx).unwrap(),
                        )
                    }
                };
            });
        });

        Ok(promise.into_value())
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
#[cfg(not(feature = "deferred-resolution"))]
fn resolve<'js>(_ctx: Ctx<'js>, func: Function<'js>, value: Value<'js>) {
    if let Err(error) = func.call::<_, Value>((value,)) {
        eprintln!("Error when promise resolution: {}", error);
    }
}

#[cfg(feature = "deferred-resolution")]
fn resolve<'js>(ctx: Ctx<'js>, func: Function<'js>, value: Value<'js>) {
    let args = [func.0.as_js_value(), value.as_js_value()];
    unsafe {
        qjs::JS_EnqueueJob(
            ctx.ctx,
            Some(resolution_job),
            args.len() as _,
            args.as_ptr() as _,
        );
    }
}

#[cfg(feature = "deferred-resolution")]
unsafe extern "C" fn resolution_job(
    ctx: *mut qjs::JSContext,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
) -> qjs::JSValue {
    let this = qjs::JS_GetGlobalObject(ctx);
    let func = *argv;
    let argv = argv.offset(1);
    let argc = argc - 1;
    qjs::JS_Call(ctx, func, this, argc, argv)
}

#[cfg(all(test, any(feature = "async-std", feature = "tokio")))]
mod test {
    use crate::*;
    use futures_rs::prelude::*;

    macro_rules! test_cases {
	      ($($name:ident ($ctx:ident) { $($content:tt)* })*) => {
            #[cfg(feature = "tokio")]
            mod tokio_tests {
                use super::*;

                async fn delayed<T>(msec: u32, value: T) -> T {
                    let dur = std::time::Duration::from_millis(msec as _);
                    tokio_rs::time::sleep(dur).await;
                    value
                }

                $(
                    #[tokio::test]
                    async fn $name() {
                        #[cfg(not(feature = "parallel"))]
                        {
                            tokio::task::LocalSet::new().run_until(async {
                                let rt = Runtime::new().unwrap().into_async(Tokio);
                                let $ctx = Context::full(&rt).unwrap();

                                rt.spawn_pending_jobs(None);

                                $($content)*
                            }).await;
                        }
                        #[cfg(feature = "parallel")]
                        {
                            let rt = Runtime::new().unwrap().into_async(Tokio);
                            let $ctx = Context::full(&rt).unwrap();

                            rt.spawn_pending_jobs(None);

                            $($content)*
                        }
                    }
                )*
            }

            #[cfg(feature = "async-std")]
            mod async_std_tests {
                use super::*;

                async fn delayed<T>(msec: u32, value: T) -> T {
                    let dur = std::time::Duration::from_millis(msec as _);
                    async_std_rs::task::sleep(dur).await;
                    value
                }
                $(
                    #[async_std::test]
                    async fn $name() {
                        let rt = Runtime::new().unwrap().into_async(AsyncStd);
                        let $ctx = Context::full(&rt).unwrap();

                        rt.spawn_pending_jobs(None);

                        $($content)*
                    }
                )*
            }
	      };
    }

    test_cases! {
        delayed_fn (ctx) {
            let res: Promise<i32> = ctx.with(|ctx| {
                let global = ctx.globals();
                global
                    .set(
                        "delayed",
                        Func::from(|msec, data: i32| PromiseJs(delayed(msec, data))),
                    )
                    .unwrap();
                ctx.eval("delayed(50, 2)").unwrap()
            });

            let res2 = async { res.await.unwrap() }.into_stream();
            let res1 = delayed(25, 1).into_stream();
            let res3 = delayed(75, 3).into_stream();

            let res = res1.chain(res2).chain(res3).collect::<Vec<_>>().await;
            assert_eq!(res, &[1, 2, 3]);
        }

        async_fn_no_throw (ctx) {
            async fn mul2(a: i32, b: i32) -> i32 {
                a * b
            }

            let res: Promise<i32> = ctx.with(|ctx| {
                let global = ctx.globals();
                global
                    .set("mul2", Func::from(|a, b| PromiseJs(mul2(a, b))))
                    .unwrap();
                ctx.eval("mul2(2, 3)").unwrap()
            });

            let res = res.await.unwrap();
            assert_eq!(res, 6);
        }

        unhandled_promise_js (ctx) {
            async fn doit() {}

            ctx.with(|ctx| {
                let global = ctx.globals();
                global
                    .set("doit", Func::from(|| PromiseJs(doit())))
                    .unwrap();
                let _ = ctx.eval::<Value, _>("doit()").unwrap();
            });

            delayed(1, 0).await;
        }

        unhandled_promise_future (ctx) {
            async fn doit() {}

            let _res: Promise<()> = ctx.with(|ctx| {
                let global = ctx.globals();
                global
                    .set("doit", Func::from(|| PromiseJs(doit())))
                    .unwrap();
                ctx.eval("doit()").unwrap()
            });

            delayed(1, 0).await;
        }
    }
}
