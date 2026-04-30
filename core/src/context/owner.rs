use core::ptr::NonNull;

use crate::qjs;

#[cfg(feature = "parallel")]
use std::sync::Arc;

/// Trait to specify how to drop a context once it goes out of scope.
/// Implemented on Runtime and AsyncRuntime.
pub(crate) trait DropContext: Clone {
    unsafe fn drop_context(&self, ctx: NonNull<qjs::JSContext>);
}

#[cfg(feature = "parallel")]
unsafe impl<R: Send + DropContext> Send for ContextOwner<R> {}

/// Newtype so `Arc<CtxPtr>` is `Send + Sync` without tripping
/// `clippy::arc_with_non_send_sync`. Access to the underlying QuickJS context
/// is protected by the runtime lock, so sharing the pointer between threads
/// is sound as long as callers go through `ContextOwner`.
#[cfg(feature = "parallel")]
#[repr(transparent)]
pub(crate) struct CtxPtr(pub(crate) NonNull<qjs::JSContext>);

#[cfg(feature = "parallel")]
unsafe impl Send for CtxPtr {}
#[cfg(feature = "parallel")]
unsafe impl Sync for CtxPtr {}

/// Struct in charge of dropping contexts when they go out of scope
pub(crate) struct ContextOwner<R: DropContext> {
    #[cfg(not(feature = "parallel"))]
    ctx: NonNull<qjs::JSContext>,
    #[cfg(feature = "parallel")]
    pub(crate) ctx: Arc<CtxPtr>,
    pub(crate) rt: R,
}

impl<R: DropContext> ContextOwner<R> {
    #[cfg(not(feature = "parallel"))]
    pub(crate) unsafe fn new(ctx: NonNull<qjs::JSContext>, rt: R) -> Self {
        Self { ctx, rt }
    }
    #[cfg(feature = "parallel")]
    pub(crate) unsafe fn new(ctx: NonNull<qjs::JSContext>, rt: R) -> Self {
        Self {
            ctx: Arc::new(CtxPtr(ctx)),
            rt,
        }
    }

    #[cfg(not(feature = "parallel"))]
    pub(crate) fn ctx(&self) -> NonNull<qjs::JSContext> {
        self.ctx
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn ctx(&self) -> NonNull<qjs::JSContext> {
        self.ctx.0
    }

    pub(crate) fn rt(&self) -> &R {
        &self.rt
    }
}

#[cfg(not(feature = "parallel"))]
impl<R: DropContext> Clone for ContextOwner<R> {
    fn clone(&self) -> Self {
        let ctx = unsafe { NonNull::new_unchecked(qjs::JS_DupContext(self.ctx.as_ptr())) };
        let rt = self.rt.clone();
        Self { ctx, rt }
    }
}

#[cfg(feature = "parallel")]
impl<R: DropContext> Clone for ContextOwner<R> {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
            rt: self.rt.clone(),
        }
    }
}

#[cfg(not(feature = "parallel"))]
impl<R: DropContext> Drop for ContextOwner<R> {
    fn drop(&mut self) {
        unsafe { self.rt.drop_context(self.ctx()) }
    }
}

#[cfg(feature = "parallel")]
impl<R: DropContext> Drop for ContextOwner<R> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.ctx) == 1 {
            unsafe { self.rt.drop_context(self.ctx()) }
        }
    }
}
