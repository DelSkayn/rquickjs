#[cfg(all(not(feature = "parallel"), not(feature = "futures")))]
mod base;
#[cfg(not(any(feature = "parallel", feature = "futures")))]
pub use base::*;

#[cfg(all(feature = "parallel", not(feature = "futures")))]
mod parallel;
#[cfg(all(feature = "parallel", not(feature = "futures")))]
pub use parallel::*;

#[cfg(feature = "futures")]
mod future;
#[cfg(feature = "futures")]
pub use future::*;
