//! Quickjs runtime related types.

pub(crate) mod raw;

mod base;
pub use base::{Runtime, WeakRuntime};

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};

pub use crate::qjs::JSMemoryUsage as MemoryUsage;
