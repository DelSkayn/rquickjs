use std::ops::Deref;
#[cfg(feature = "parallel")]
use std::sync::Arc;

/// A wrapper around an inner type which implements cloning with the underlying type if parallel
/// feature is disabled and with an Arc otherwise.
#[repr(transparent)]
pub struct ContextRef<Ctx> {
    #[cfg(not(feature = "parallel"))]
    ctx: Ctx,
    #[cfg(feature = "parallel")]
    ctx: Arc<Ctx>,
}

#[cfg(feature = "parallel")]
impl<Ctx> Clone for ContextRef<Ctx> {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
        }
    }
}

#[cfg(not(feature = "parallel"))]
impl<Ctx: Clone> Clone for ContextRef<Ctx> {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
        }
    }
}

impl<Ctx> ContextRef<Ctx> {
    #[cfg(feature = "parallel")]
    pub fn new(ctx: Ctx) -> Self {
        ContextRef { ctx: Arc::new(ctx) }
    }

    #[cfg(not(feature = "parallel"))]
    pub fn new(ctx: Ctx) -> Self {
        ContextRef { ctx }
    }
}

impl<Ctx> Deref for ContextRef<Ctx> {
    type Target = Ctx;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
