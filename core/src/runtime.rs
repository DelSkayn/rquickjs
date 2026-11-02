//! QuickJS runtime related types.

mod base;
mod exotic;
pub(crate) mod opaque;
pub(crate) mod raw;
mod userdata;

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "futures")]
pub(crate) mod schedular;
#[cfg(feature = "futures")]
mod spawner;
#[cfg(feature = "futures")]
pub use spawner::DriveFuture;

use alloc::{boxed::Box, vec::Vec};
pub use base::{Runtime, WeakRuntime};
pub use userdata::{UserDataError, UserDataGuard};

#[cfg(feature = "futures")]
pub(crate) use r#async::InnerRuntime;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};

use crate::value::promise::PromiseHookType;
use crate::{Ctx, Error, Result, Value};

#[cfg(feature = "parallel")]
#[derive(Clone)]
pub(crate) struct JobErrorQueue(std::sync::Arc<std::sync::Mutex<Vec<u64>>>);
#[cfg(not(feature = "parallel"))]
#[derive(Clone)]
pub(crate) struct JobErrorQueue(alloc::rc::Rc<core::cell::RefCell<Vec<u64>>>);

impl JobErrorQueue {
    pub(crate) fn new() -> Self {
        #[cfg(feature = "parallel")]
        {
            Self(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
        }
        #[cfg(not(feature = "parallel"))]
        {
            Self(alloc::rc::Rc::new(core::cell::RefCell::new(Vec::new())))
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        #[cfg(feature = "parallel")]
        {
            self.0.lock().unwrap().is_empty()
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.0.borrow().is_empty()
        }
    }

    pub(crate) fn push(&self, id: u64) {
        #[cfg(feature = "parallel")]
        self.0.lock().unwrap().push(id);
        #[cfg(not(feature = "parallel"))]
        self.0.borrow_mut().push(id);
    }

    pub(crate) fn take(&self) -> Vec<u64> {
        #[cfg(feature = "parallel")]
        {
            core::mem::take(&mut *self.0.lock().unwrap())
        }
        #[cfg(not(feature = "parallel"))]
        {
            core::mem::take(&mut *self.0.borrow_mut())
        }
    }
}

/// A failed job.
///
/// The exception stays in the runtime until [`JobError::into_value`] takes it
/// with a context of the same runtime.
pub struct JobError {
    id: u64,
    released: JobErrorQueue,
}

impl JobError {
    pub(crate) fn new(id: u64, released: JobErrorQueue) -> Self {
        Self { id, released }
    }

    /// Takes the failed job's exception out of the runtime.
    pub fn into_value<'js>(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let this = core::mem::ManuallyDrop::new(self);
        let opaque = unsafe { ctx.get_opaque() };
        let persistent = opaque.take_job_error(this.id).ok_or(Error::Exception)?;
        persistent.restore(ctx)
    }
}

impl Drop for JobError {
    fn drop(&mut self) {
        self.released.push(self.id);
    }
}

impl core::fmt::Debug for JobError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("JobError").finish_non_exhaustive()
    }
}

/// The type of the promise hook.
#[cfg(not(feature = "parallel"))]
pub type PromiseHook =
    Box<dyn for<'a> Fn(Ctx<'a>, PromiseHookType, Value<'a>, Value<'a>) + 'static>;
/// The type of the promise hook.
#[cfg(feature = "parallel")]
pub type PromiseHook =
    Box<dyn for<'a> Fn(Ctx<'a>, PromiseHookType, Value<'a>, Value<'a>) + Send + 'static>;

/// The type of the promise rejection tracker.
#[cfg(not(feature = "parallel"))]
pub type RejectionTracker = Box<dyn for<'a> Fn(Ctx<'a>, Value<'a>, Value<'a>, bool) + 'static>;
/// The type of the promise rejection tracker.
#[cfg(feature = "parallel")]
pub type RejectionTracker =
    Box<dyn for<'a> Fn(Ctx<'a>, Value<'a>, Value<'a>, bool) + Send + 'static>;

/// The type of the interrupt handler.
#[cfg(not(feature = "parallel"))]
pub type InterruptHandler = Box<dyn FnMut() -> bool + 'static>;
/// The type of the interrupt handler.
#[cfg(feature = "parallel")]
pub type InterruptHandler = Box<dyn FnMut() -> bool + Send + 'static>;

/// A struct with information about the runtimes memory usage.
pub type MemoryUsage = crate::qjs::JSMemoryUsage;
