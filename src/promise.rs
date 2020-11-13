use crate::{Ctx, Error, FromJs, Function, Object, Result, Runtime, StdResult, ToJs, Value};
use rquickjs_sys as qjs;
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

macro_rules! fromjs_for_promise {
    ($($extra_guards: tt)*) => {
        impl<'js, T> FromJs<'js> for Promise<T>
        where
            T: FromJs<'js> + 'static,
            $(T: $extra_guards,)*
        {
            fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                let obj = Object::from_js(ctx, value)?;
                let then: Function = obj.get("then")?;
                let state = Arc::new(Mutex::new(State::default()));
                let on_ok = Function::new(ctx, "onSuccess", {
                    let state = state.clone();
                    move |ctx, _this: Value, (value,): (Value,)| {
                        let mut state = state.lock().unwrap();
                        state.result = T::from_js(ctx, value).into();
                        if let Some(waker) = state.waker.take() {
                            waker.wake();
                        }
                        Ok(())
                    }
                })?;
                let on_err = Function::new(ctx, "onError", {
                    let state = state.clone();
                    move |_ctx, _this: Value, (error,): (Error,)| {
                        let mut state = state.lock().unwrap();
                        state.result = Err(error).into();
                        if let Some(waker) = state.waker.take() {
                            waker.wake();
                        }
                        Ok(())
                    }
                })?;
                then.call_on(obj, (on_ok, on_err))?;
                Ok(Self { state })
            }
        }
    };
}

#[cfg(not(feature = "parallel"))]
fromjs_for_promise!();
#[cfg(feature = "parallel")]
fromjs_for_promise!(Send);

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
pub struct PromiseJs<T> {
    runtime: Runtime,
    future: T,
}

impl<T> PromiseJs<T> {
    fn new(runtime: Runtime, future: T) -> Self {
        Self { runtime, future }
    }
}

#[cfg(any(feature = "async-std", feature = "tokio"))]
impl<'js, 'a, T, V, E> ToJs<'js> for PromiseJs<T>
where
    T: Future<Output = StdResult<V, E>> + 'static,
    V: ToJs<'js> + 'static,
    E: Display + 'static,
{
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        #[cfg(feature = "async-std")]
        use async_std_rs::task::spawn_local as spawn;
        #[cfg(feature = "tokio")]
        use tokio_rs::task::spawn_local as spawn;

        let (promise, then, catch) = ctx.promise()?;

        let then = ctx.register(Value::Function(then));
        let catch = ctx.register(Value::Function(catch));

        let ctx = ctx.ctx;
        let Self { runtime, future } = self;

        spawn(async move {
            let rt_lock = runtime.inner.lock();
            let ctx = Ctx::from_ptr(ctx);
            let then = ctx.deregister(then).unwrap();
            let catch = ctx.deregister(catch).unwrap();
            match future.await {
                Ok(value) => match value.to_js(ctx) {
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

fn schedule_resolution<'js, V: ToJs<'js>>(ctx: Ctx<'js>, func: Value<'js>, val: V) {
    if let Ok(val) = val.to_js(ctx) {
        let args = [func.as_js_value(), val.as_js_value()];
        unsafe {
            qjs::JS_EnqueueJob(
                ctx.ctx,
                Some(resolution_job),
                args.len() as i32,
                args.as_ptr() as *mut _,
            );
        }
        mem::drop(args);
    }
}

unsafe extern "C" fn resolution_job(
    ctx: *mut qjs::JSContext,
    argc: std::os::raw::c_int,
    argv: *mut qjs::JSValue,
) -> qjs::JSValue {
    let this = qjs::JS_GetGlobalObject(ctx);
    let func = *argv;
    let argv = argv.offset(1);
    let argc = argc - 1;
    qjs::JS_Call(ctx, func, this, argc, argv)
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
impl Runtime {
    /// Create promise from future
    pub fn promise<'js, T, V, E>(&self, future: T) -> PromiseJs<T>
    where
        T: Future<Output = StdResult<V, E>> + 'static,
        V: ToJs<'js>,
        E: Display,
    {
        PromiseJs::new(self.clone(), future)
    }
}
