//! QuickJS runtime related types.

mod base;
pub(crate) mod opaque;
pub(crate) mod raw;
mod userdata;

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "futures")]
pub(crate) mod schedular;
#[cfg(feature = "futures")]
mod spawner;

pub use base::{Runtime, WeakRuntime};
pub use userdata::{UserDataError, UserDataGuard};

#[cfg(feature = "futures")]
pub(crate) use r#async::InnerRuntime;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};

use crate::{Ctx, Value};

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
