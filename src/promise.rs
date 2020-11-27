use crate::{
    qjs, Ctx, Error, FromJs, Function, IntoJs, Object, Result, SendWhenParallel, StdResult, This,
    Value,
};
use std::{
    fmt::Display,
    future::Future,
    mem,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

/// Future-aware promise
pub struct Promise<T> {
    state: Arc<Mutex<State<T>>>,
}

struct State<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
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
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js(ctx, value)?;
        let then: Function = obj.get("then")?;
        let state = Arc::new(Mutex::new(State::default()));
        let on_ok = Function::new(ctx, "onSuccess", {
            let state = state.clone();
            move |ctx: Ctx<'js>, value: Value<'js>| {
                let mut state = state.lock().unwrap();
                state.result = Some(T::from_js(ctx, value));
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
        })?;
        let on_err = Function::new(ctx, "onError", {
            let state = state.clone();
            move |error: Error| {
                let mut state = state.lock().unwrap();
                state.result = Some(Err(error));
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
        })?;
        then.call((This(obj), on_ok, on_err))?;
        Ok(Self { state })
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        if let Some(result) = state.result.take() {
            return Poll::Ready(result);
        }
        state.waker = cx.waker().clone().into();
        Poll::Pending
    }
}

/// Wrapper for futures to convert to JS promises
#[repr(transparent)]
pub struct PromiseJs<T>(pub T);

impl<T> From<T> for PromiseJs<T> {
    fn from(future: T) -> Self {
        Self(future)
    }
}

#[cfg(any(feature = "async-std", feature = "tokio"))]
impl<'js, 'a, T, V, E> IntoJs<'js> for PromiseJs<T>
where
    T: Future<Output = StdResult<V, E>> + 'static,
    V: IntoJs<'js> + 'static,
    E: Display + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        #[cfg(feature = "async-std")]
        use async_std_rs::task::spawn_local as spawn;
        #[cfg(feature = "tokio")]
        use tokio_rs::task::spawn_local as spawn;

        let (promise, then, catch) = ctx.promise()?;

        let then = ctx.register(Value::Function(then));
        let catch = ctx.register(Value::Function(catch));

        let runtime = unsafe { &ctx.get_opaque().runtime }
            .try_ref()
            .ok_or(Error::Unknown)?;

        let ctx = ctx.ctx;
        let future = self.0;

        spawn(async move {
            let result = future.await;

            let rt_lock = runtime.inner.lock();

            let ctx = Ctx::from_ptr(ctx);
            let then = ctx.deregister(then).unwrap();
            let catch = ctx.deregister(catch).unwrap();

            match result {
                Ok(value) => match value.into_js(ctx) {
                    Ok(value) => schedule_resolution(ctx, then, value),
                    Err(error) => schedule_resolution(ctx, catch, error.to_string()),
                },
                Err(error) => schedule_resolution(ctx, catch, error.to_string()),
            };

            mem::drop(rt_lock);
        });

        Ok(Value::Object(promise))
    }
}

fn schedule_resolution<'js, V: IntoJs<'js>>(ctx: Ctx<'js>, func: Value<'js>, val: V) {
    if let Ok(val) = val.into_js(ctx) {
        let args = [func.as_js_value(), val.as_js_value()];
        unsafe {
            qjs::JS_EnqueueJob(
                ctx.ctx,
                Some(resolution_job),
                args.len() as i32,
                args.as_ptr() as *mut _,
            );
        }
    }
}

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
