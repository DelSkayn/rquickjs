use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

use crate::{Context, Ctx, FromJs, IntoJs, ParallelSend, Result, Value, markers::Invariant, qjs};

/// A promise which can be awaited as a future.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Promise<T> {
    _tmp: PhantomData<T>,
}

impl<'js, T> FromJs<'js> for Promise<T>
where
    T: FromJs<'js> + ParallelSend,
{
    fn from_js(_ctx: Ctx<'js>, _value: Value<'js>) -> Result<Self> {
        todo!()
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext) -> Poll<Self::Output> {
        todo!()
    }
}

/// Wrapper for futures to convert to JS promises
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
#[repr(transparent)]
pub struct Promised<T>(pub T);

impl<'js, T> IntoJs<'js> for Promised<T>
where
    T: Future + ParallelSend + 'js,
    for<'js_> T::Output: IntoJs<'js_> + 'js,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (promise, _then, _catch) = ctx.promise()?;

        let context = Context::from_ctx(ctx).unwrap();
        

        Ok(promise.into_value())
    }
}

pub struct PromisedTask<'js>{
    ptr: *mut qjs::JSContext,
    _maker: Invariant<'js>
}

impl Drop for PromisedTask<'js>{
    fn drop(&mut self) {
        qjs::JS_D self.ptr
    }
}
