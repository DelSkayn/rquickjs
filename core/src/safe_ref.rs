#[cfg(not(any(feature = "parallel", feature = "tokio", feature = "async-std")))]
mod base;
#[cfg(not(any(feature = "parallel", feature = "tokio", feature = "async-std")))]
pub use base::*;

#[cfg(all(not(feature = "tokio"), feature = "async-std"))]
mod future_async;
#[cfg(all(not(feature = "tokio"), feature = "async-std"))]
pub use future_async::*;

#[cfg(feature = "tokio")]
mod future_tokio;
#[cfg(feature = "tokio")]
pub use future_tokio::*;

#[cfg(all(
    feature = "parallel",
    not(any(feature = "tokio", feature = "async-std"))
))]
mod parallel;
#[cfg(all(
    feature = "parallel",
    not(any(feature = "tokio", feature = "async-std"))
))]
pub use parallel::*;
