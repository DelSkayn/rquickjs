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

use crate::{atom::PredefinedAtom, qjs, Ctx, Error, FromJs, Function, Object, Result, Value};
#[cfg(feature = "futures")]
use crate::{function::This, CatchResultExt, CaughtError, IntoJs};

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
    pub fn finish<T: FromJs<'js> + std::fmt::Debug>(&self) -> Result<T> {
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
