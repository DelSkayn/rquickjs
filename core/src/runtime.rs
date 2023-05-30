//! Quickjs runtime related types.

pub(crate) mod raw;

mod base;
pub use base::{Runtime, WeakRuntime};

pub use crate::qjs::JSMemoryUsage as MemoryUsage;
