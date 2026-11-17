use alloc::boxed::Box;
use core::{mem, ptr::NonNull};

use crate::{context::opaque::ContextOpaque, qjs, Ref};

pub struct OwnedContext {
    ctx: NonNull<qjs::JSContext>,
    /// Whether this handle owns the `ContextOpaque` installed on `ctx`.
    /// `from_raw` wraps a context whose opaque belongs to another `Context`.
    owns_opaque: bool,
}

impl OwnedContext {
    pub fn new(ctx: NonNull<qjs::JSContext>, opaque: ContextOpaque) -> Self {
        debug_assert!(
            unsafe { qjs::JS_GetContextOpaque(ctx.as_ptr()) }.is_null(),
            "Tried to create a rust quickjs context from a context which already contained an opaque pointer"
        );

        let opaque = Box::into_raw(Box::new(opaque));

        unsafe { qjs::JS_SetContextOpaque(ctx.as_ptr(), opaque.cast()) };

        OwnedContext {
            ctx,
            owns_opaque: true,
        }
    }

    /// Wrap an existing rquickjs context. A fresh opaque is installed only when
    /// the context does not have one yet; otherwise its original owner keeps it.
    ///
    /// # Safety
    /// `ctx` must have a reference count which can be decremented on drop.
    pub unsafe fn from_raw(ctx: NonNull<qjs::JSContext>) -> Self {
        if qjs::JS_GetContextOpaque(ctx.as_ptr()).is_null() {
            Self::new(ctx, ContextOpaque::new())
        } else {
            OwnedContext {
                ctx,
                owns_opaque: false,
            }
        }
    }
}

impl Drop for OwnedContext {
    fn drop(&mut self) {
        unsafe {
            if self.owns_opaque {
                mem::drop(Box::from_raw(
                    qjs::JS_GetContextOpaque(self.ctx.as_ptr()).cast::<ContextOpaque>(),
                ));
            }
            qjs::JS_FreeContext(self.ctx.as_ptr());
        }
    }
}

/// Trait to specify how to drop a context once it goes out of scope.
/// Implemented on Runtime and AsyncRuntime.
pub(crate) trait DropContext: Clone {
    unsafe fn drop_context(&self, ctx: OwnedContext);
}

impl DropContext for crate::Runtime {
    unsafe fn drop_context(&self, ctx: OwnedContext) {
        //TODO
        let guard = match self.inner.try_lock() {
            Some(x) => x,
            None => {
                // `RefCell` is neither `Send` nor `Sync`, so a failed
                // `try_borrow_mut` is always this same thread and freeing
                // directly is safe.
                #[cfg(not(feature = "parallel"))]
                {
                    mem::drop(ctx);
                    return;
                }
                #[cfg(feature = "parallel")]
                {
                    self.pending_free
                        .send(ctx)
                        .expect("runtime should be alive while contexts life");
                    return;
                }
            }
        };
        guard.update_stack_top();
        mem::drop(ctx);
        guard.drain_pending_free();
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }
}

#[cfg(feature = "futures")]
impl DropContext for crate::AsyncRuntime {
    unsafe fn drop_context(&self, ctx: OwnedContext) {
        //TODO
        let guard = match self.inner.try_lock() {
            Some(x) => x,
            None => {
                #[cfg(not(feature = "parallel"))]
                {
                    // `RefCell` is neither `Send` nor `Sync`, so a failed
                    // `try_borrow_mut` is always this same thread and freeing
                    // directly is safe.
                    mem::drop(ctx);
                    return;
                }
                #[cfg(feature = "parallel")]
                {
                    self.pending_free
                        .send(ctx)
                        .expect("runtime should be alive while contexts life");
                    return;
                }
            }
        };
        guard.runtime.update_stack_top();
        mem::drop(ctx);
        guard.runtime.drain_pending_free();
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
    }
}

/// Struct in charge of dropping contexts when they go out of scope.
///
/// The context is shared through `Ref`, so only the last handle frees the C
/// context, and it does so through `DropContext` while the runtime lock is
/// held. `Option` lets `Drop` take the `Ref` out of `&mut self` without an
/// `unsafe` `ManuallyDrop`.
#[derive(Clone)]
pub(crate) struct ContextOwner<R: DropContext> {
    ctx: Option<Ref<OwnedContext>>,
    pub(crate) rt: R,
}

impl<R: DropContext> ContextOwner<R> {
    pub unsafe fn new(ctx: NonNull<qjs::JSContext>, opaque: ContextOpaque, rt: R) -> Self {
        Self {
            ctx: Some(Ref::new(OwnedContext::new(ctx, opaque))),
            rt,
        }
    }

    /// # Safety
    /// `ctx` must have a reference count which can be decremented on drop.
    pub(crate) unsafe fn from_raw(ctx: NonNull<qjs::JSContext>, rt: R) -> Self {
        Self {
            ctx: Some(Ref::new(OwnedContext::from_raw(ctx))),
            rt,
        }
    }

    pub(crate) fn ctx(&self) -> NonNull<qjs::JSContext> {
        self.ctx
            .as_ref()
            .expect("context owner is already dropped")
            .ctx
    }

    pub(crate) fn rt(&self) -> &R {
        &self.rt
    }
}

// Access to the underlying QuickJS context is protected by the runtime lock,
// so sharing this pointer between threads is sound as long as callers go
// through `ContextOwner`.
#[cfg(feature = "parallel")]
unsafe impl Send for OwnedContext {}
#[cfg(feature = "parallel")]
unsafe impl Sync for OwnedContext {}
#[cfg(feature = "parallel")]
unsafe impl<R: Send + DropContext> Send for ContextOwner<R> {}

impl<R: DropContext> Drop for ContextOwner<R> {
    fn drop(&mut self) {
        if let Some(inner) = self.ctx.take() {
            if let Some(inner) = Ref::into_inner(inner) {
                unsafe { self.rt.drop_context(inner) }
            }
        }
    }
}
