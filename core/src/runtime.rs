//! QuickJS runtime related types.

#[cfg(feature = "futures")]
mod r#async;
mod base;
pub(crate) mod raw;
#[cfg(feature = "futures")]
pub(crate) mod schedular;

pub use base::{Runtime, WeakRuntime};
#[cfg(feature = "futures")]
pub(crate) use r#async::InnerRuntime;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};
#[cfg(feature = "futures")]
mod spawner;

/// The type of the interrupt handler.
#[cfg(not(feature = "parallel"))]
pub type InterruptHandler = Box<dyn FnMut() -> bool + 'static>;
/// The type of the interrupt handler.
#[cfg(feature = "parallel")]
pub type InterruptHandler = Box<dyn FnMut() -> bool + Send + 'static>;

/// A struct with information about the runtimes memory usage.
pub type MemoryUsage = crate::qjs::JSMemoryUsage;
