//! Javascript promises and future integration.
#[cfg(feature = "futures")]
use std::{
    cell::RefCell,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use crate::{
    atom::PredefinedAtom, qjs, Ctx, Error, FromJs, Function, IntoJs, Object, Result, Value,
};
#[cfg(feature = "futures")]
use crate::{function::This, CatchResultExt, CaughtError};

/// The execution state of a promise.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum PromiseState {
    /// The promise has not yet completed.
    Pending,
    /// The promise completed succefully.
    Resolved,
    /// The promise completed with an error.
    Rejected,
}

/// A JavaScript promise.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
#[repr(transparent)]
pub struct Promise<'js>(pub(crate) Object<'js>);

impl<'js> Promise<'js> {
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[cfg(feature = "futures")]
    pub fn wrap_future<F, R>(ctx: &Ctx<'js>, future: F) -> Result<Self>
    where
        F: Future<Output = R> + 'js,
        R: IntoJs<'js>,
    {
        let (promise, resolve, reject) = ctx.promise()?;
        let ctx_clone = ctx.clone();
        let future = async move {
            let err = match future.await.into_js(&ctx_clone).catch(&ctx_clone) {
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
        Ok(promise)
    }

    /// Create a new JavaScript promise along with its resolve and reject functions.
    pub fn new(ctx: &Ctx<'js>) -> Result<(Self, Function<'js>, Function<'js>)> {
        ctx.promise()
    }

    /// Returns the state of the promise, either pending,resolved or rejected.
    pub fn state(&self) -> PromiseState {
        let v = unsafe { qjs::JS_PromiseState(self.ctx().as_ptr(), self.as_js_value()) };
        match v {
            qjs::JSPromiseStateEnum_JS_PROMISE_PENDING => PromiseState::Pending,
            qjs::JSPromiseStateEnum_JS_PROMISE_FULFILLED => PromiseState::Resolved,
            qjs::JSPromiseStateEnum_JS_PROMISE_REJECTED => PromiseState::Rejected,
            _ => unreachable!(),
        }
    }

    /// Returns the `then` function, used for chaining promises.
    pub fn then(&self) -> Result<Function<'js>> {
        self.get(PredefinedAtom::Then)
    }

    /// Returns the `catch` function, used for retrieving the result of a rejected promise.
    pub fn catch(&self) -> Result<Function<'js>> {
        self.get(PredefinedAtom::Catch)
    }

    /// Returns the result of the future if there is one.
    ///
    /// Returns None if the promise has not yet been completed, Ok if the promise was resolved, and
    /// [`Error::Exception`] if the promise rejected with the rejected value as the thrown
    /// value retrievable via [`Ctx::catch`].
    pub fn result<T: FromJs<'js>>(&self) -> Option<Result<T>> {
        match self.state() {
            PromiseState::Pending => None,
            PromiseState::Resolved => {
                let v = unsafe { qjs::JS_PromiseResult(self.ctx().as_ptr(), self.as_js_value()) };
                let v = unsafe { Value::from_js_value(self.ctx().clone(), v) };
                Some(FromJs::from_js(self.ctx(), v))
            }
            PromiseState::Rejected => {
                unsafe {
                    let v = qjs::JS_PromiseResult(self.ctx().as_ptr(), self.as_js_value());
                    qjs::JS_Throw(self.ctx().as_ptr(), v);
                };
                Some(Err(Error::Exception))
            }
        }
    }

    /// Runs the quickjs job queue until the promise is either rejected or resolved.
    ///
    /// If blocking on the promise would result in blocking, i.e. when the job queue runs out of
    /// jobs before the promise can be resolved, this function returns [`Error::WouldBlock`]
    /// indicating that no more work can be done at the momement.
    ///
    /// This function only drives the quickjs job queue, futures are not polled.
    pub fn finish<T: FromJs<'js>>(&self) -> Result<T> {
        loop {
            if let Some(x) = self.result() {
                return x;
            }

            if !self.ctx.execute_pending_job() {
                return Err(Error::WouldBlock);
            }
        }
    }

    /// Wrap the promise into a struct which can be polled as a rust future.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[cfg(feature = "futures")]
    pub fn into_future<T>(self) -> PromiseFuture<'js, T>
    where
        T: FromJs<'js>,
    {
        PromiseFuture {
            state: None,
            promise: self,
            _marker: PhantomData,
        }
    }
}

/// Future-aware promise
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[cfg(feature = "futures")]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct PromiseFuture<'js, T> {
    state: Option<Rc<RefCell<Waker>>>,
    promise: Promise<'js>,
    _marker: PhantomData<T>,
}

// Nothing is actually pinned so promise future is unpin.
#[cfg(feature = "futures")]
impl<'js, T> Unpin for PromiseFuture<'js, T> {}

#[cfg(feature = "futures")]
impl<'js, T> Future for PromiseFuture<'js, T>
where
    T: FromJs<'js>,
{
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if let Some(x) = this.promise.result() {
            return Poll::Ready(x);
        }

        if this.state.is_none() {
            let inner = Rc::new(RefCell::new(cx.waker().clone()));
            this.state = Some(inner.clone());

            let resolve = Function::new(this.promise.ctx.clone(), move || {
                inner.borrow().wake_by_ref();
            })?;

            this.promise
                .then()?
                .call((This(this.promise.clone()), resolve.clone(), resolve))?;
            return Poll::Pending;
        }

        this.state
            .as_ref()
            .unwrap()
            .borrow_mut()
            .clone_from(cx.waker());

        Poll::Pending
    }
}

/// Wrapper for futures to convert to JS promises
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
#[cfg(feature = "futures")]
pub struct Promised<T>(pub T);

#[cfg(feature = "futures")]
impl<T> From<T> for Promised<T> {
    fn from(future: T) -> Self {
        Self(future)
    }
}

#[cfg(feature = "futures")]
impl<'js, T, R> IntoJs<'js> for Promised<T>
where
    T: Future<Output = R> + 'js,
    R: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Promise::wrap_future(ctx, self.0).map(|x| x.into_value())
    }
}

/// A type which behaves like a promise but can wrap any javascript value.
///
/// This type is usefull when you are unsure if a function will return a promise.
/// You can call finish and turn it into a future like a normal promise.
/// When the value this type us converted isn't a promise it will behave like an promise which is
/// already resolved, otherwise it will call the right functions on the promise.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct MaybePromise<'js>(Value<'js>);

impl<'js> FromJs<'js> for MaybePromise<'js> {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(MaybePromise(value))
    }
}

impl<'js> IntoJs<'js> for MaybePromise<'js> {
    fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.0)
    }
}

impl<'js> MaybePromise<'js> {
    /// Reference to the inner value
    pub fn as_value(&self) -> &Value<'js> {
        &self.0
    }

    /// Convert into the inner value
    pub fn into_value(self) -> Value<'js> {
        self.0
    }

    /// Convert into the inner value
    pub fn from_value(value: Value<'js>) -> Self {
        MaybePromise(value)
    }

    /// Returns the [`Ctx`] object associated with this value
    pub fn ctx(&self) -> &Ctx<'js> {
        self.0.ctx()
    }

    /// Returns [`PromiseState::Resolved`] if the wrapped value isn't a promise, otherwise calls
    /// [`Promise::state`] on the promise and returns it's value.
    pub fn state(&self) -> PromiseState {
        if let Some(x) = self.0.as_promise() {
            x.state()
        } else {
            PromiseState::Resolved
        }
    }

    /// Returns the value if self isn't a promise, otherwise calls [`Promise::result`] on the promise.
    pub fn result<T: FromJs<'js>>(&self) -> Option<Result<T>> {
        if let Some(x) = self.0.as_promise() {
            x.result::<T>()
        } else {
            Some(T::from_js(self.0.ctx(), self.0.clone()))
        }
    }

    /// Returns the value if self isn't a promise, otherwise calls [`Promise::finish`] on the promise.
    pub fn finish<T: FromJs<'js>>(&self) -> Result<T> {
        if let Some(x) = self.0.as_promise() {
            x.finish::<T>()
        } else {
            T::from_js(self.0.ctx(), self.0.clone())
        }
    }

    /// Convert self into a future which will return ready if the wrapped value isn't a promise,
    /// otherwise it will handle the promise like the future returned from
    /// [`Promise::into_future`].
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[cfg(feature = "futures")]
    pub fn into_future<T: FromJs<'js>>(self) -> MaybePromiseFuture<'js, T> {
        if self.0.is_promise() {
            let fut = self.0.into_promise().unwrap().into_future();
            MaybePromiseFuture(MaybePromiseFutureInner::Future(fut))
        } else {
            MaybePromiseFuture(MaybePromiseFutureInner::Ready(self.0))
        }
    }
}

/// Future-aware maybe promise
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[cfg(feature = "futures")]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct MaybePromiseFuture<'js, T>(MaybePromiseFutureInner<'js, T>);

#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[cfg(feature = "futures")]
#[derive(Debug)]
enum MaybePromiseFutureInner<'js, T> {
    Ready(Value<'js>),
    Future(PromiseFuture<'js, T>),
}

#[cfg(feature = "futures")]
impl<'js, T> Future for MaybePromiseFuture<'js, T>
where
    T: FromJs<'js>,
{
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match self.get_mut().0 {
            MaybePromiseFutureInner::Ready(ref x) => Poll::Ready(T::from_js(x.ctx(), x.clone())),
            MaybePromiseFutureInner::Future(ref mut x) => Pin::new(x).poll(cx),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    #[cfg(feature = "futures")]
    use crate::{
        async_with,
        function::{Async, Func},
        AsyncContext, AsyncRuntime,
    };

    #[cfg(feature = "futures")]
    async fn set_timeout<'js>(cb: Function<'js>, number: f64) -> Result<()> {
        tokio::time::sleep(Duration::from_secs_f64(number / 1000.0)).await;
        cb.call::<_, ()>(())
    }

    #[cfg(feature = "futures")]
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
            let promise: Promise = func.call(()).unwrap();
            assert_eq!(promise.into_future::<i32>().await.catch(&ctx).unwrap(), 42);

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
            let promise: Promise = func.call(()).unwrap();
            let err = promise.into_future::<()>().await.catch(&ctx);
            match err {
                Err(CaughtError::Value(v)) => {
                    assert_eq!(v.as_int().unwrap(), 42)
                }
                _ => panic!(),
            }
        })
        .await
    }

    #[cfg(feature = "futures")]
    #[tokio::test]
    async fn promised() {
        use crate::Exception;

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

            function.call::<_,Promise>((promised,)).unwrap().into_future::<()>().await.unwrap();

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


            function.call::<_,Promise>((promised,)).unwrap().into_future::<()>().await.unwrap()
        })
        .await
    }
}
