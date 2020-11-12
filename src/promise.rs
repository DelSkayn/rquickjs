use crate::{Ctx, Error, FromJs, Function, Object, Result, Value};
use std::{
    future::Future,
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

impl<'js, T: FromJs<'js> + 'static> FromJs<'js> for Promise<T> {
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
