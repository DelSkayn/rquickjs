//! JS Contexts related types.

mod base;
mod builder;
mod ctx;
mod owner;

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "multi-ctx")]
mod multi_with_impl;

/// A trait for using multiple contexts at the same time.
#[cfg(feature = "multi-ctx")]
pub trait MultiWith<'js> {
    type Arg;

    /// Use multiple contexts together.
    ///
    /// # Panic
    /// This function will panic if any of the contexts are of separate runtimes.
    fn with<R, F: FnOnce(Self::Arg) -> R>(self, f: F) -> R;
}

pub use base::Context;
pub use builder::{intrinsic, ContextBuilder, Intrinsic};
pub use ctx::{Ctx, EvalOptions};

#[cfg(feature = "futures")]
pub use r#async::AsyncContext;
