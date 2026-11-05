use alloc::boxed::Box;
use core::{mem, ptr::NonNull};

use crate::{context::opaque::ContextOpaque, qjs};

pub struct OwnedContext {
    ctx: NonNull<qjs::JSContext>,
}

impl OwnedContext {
    pub fn new(ctx: NonNull<qjs::JSContext>, opaque: ContextOpaque) -> Self {
        assert!(unsafe { qjs::JS_GetContextOpaque(ctx.as_ptr()) }.is_null(), "Tried to create a rust quickjs context from a context which was already contained an opaque pointer");

        let opaque = Box::into_raw(Box::new(opaque));

        unsafe { qjs::JS_SetContextOpaque(ctx.as_ptr(), opaque.cast()) };

        OwnedContext { ctx }
    }
}

impl Drop for OwnedContext {
    fn drop(&mut self) {
        unsafe {
            let _ =
                Box::from_raw(qjs::JS_GetContextOpaque(self.ctx.as_ptr()).cast::<ContextOpaque>());
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

#[cfg(feature = "parallel")]
mod impls {
    use super::*;
    use alloc::sync::Arc;
    use core::mem::ManuallyDrop;

    /// Struct in charge of dropping contexts when they go out of scope
    #[derive(Clone)]
    pub(crate) struct ContextOwner<R: DropContext> {
        pub(crate) ctx: ManuallyDrop<Arc<OwnedContext>>,
        pub(crate) rt: R,
    }

    impl<R: DropContext> ContextOwner<R> {
        pub unsafe fn new(ctx: NonNull<qjs::JSContext>, opaque: ContextOpaque, rt: R) -> Self {
            Self {
                ctx: ManuallyDrop::new(Arc::new(OwnedContext::new(ctx, opaque))),
                rt,
            }
        }

        pub(crate) fn ctx(&self) -> NonNull<qjs::JSContext> {
            self.ctx.ctx
        }

        pub(crate) fn rt(&self) -> &R {
            &self.rt
        }
    }

    unsafe impl Send for OwnedContext {}
    unsafe impl Sync for OwnedContext {}

    unsafe impl<R: Send + DropContext> Send for ContextOwner<R> {}

    impl<R: DropContext> Drop for ContextOwner<R> {
        fn drop(&mut self) {
            let inner = unsafe { ManuallyDrop::take(&mut self.ctx) };
            if let Some(inner) = Arc::into_inner(inner) {
                unsafe { self.rt.drop_context(inner) }
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
mod impls {
    use super::*;
    use alloc::rc::Rc;
    use core::mem::ManuallyDrop;

    /// Struct in charge of dropping contexts when they go out of scope
    #[derive(Clone)]
    pub(crate) struct ContextOwner<R: DropContext> {
        ctx: ManuallyDrop<alloc::rc::Rc<OwnedContext>>,
        pub(crate) rt: R,
    }

    impl<R: DropContext> ContextOwner<R> {
        pub unsafe fn new(ctx: NonNull<qjs::JSContext>, opaque: ContextOpaque, rt: R) -> Self {
            Self {
                ctx: ManuallyDrop::new(Rc::new(OwnedContext::new(ctx, opaque))),
                rt,
            }
        }

        pub(crate) fn ctx(&self) -> NonNull<qjs::JSContext> {
            self.ctx.ctx
        }

        pub(crate) fn rt(&self) -> &R {
            &self.rt
        }
    }

    impl<R: DropContext> Drop for ContextOwner<R> {
        fn drop(&mut self) {
            let inner = unsafe { ManuallyDrop::take(&mut self.ctx) };
            if let Some(inner) = Rc::into_inner(inner) {
                unsafe { self.rt.drop_context(inner) }
            }
        }
    }
}

pub(crate) use impls::*;
