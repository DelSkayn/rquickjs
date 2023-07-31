//! Quickjs runtime related types.

pub(crate) mod raw;

mod base;
pub use base::{Runtime, WeakRuntime};

/// The type of the interrupt handler.
#[cfg(not(feature = "parallel"))]
pub type InterruptHandler = Box<dyn FnMut() -> bool + 'static>;
/// The type of the interrupt handler.
#[cfg(feature = "parallel")]
pub type InterruptHandler = Box<dyn FnMut() -> bool + Send + 'static>;

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "futures")]
pub(crate) use r#async::InnerRuntime;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};
#[cfg(feature = "futures")]
mod spawner;

pub use crate::qjs::JSMemoryUsage as MemoryUsage;
