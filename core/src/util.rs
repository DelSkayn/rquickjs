//! Module with some util types.

use core::panic::UnwindSafe;

/// A trait for preventing implementing traits which should not be implemented outside of rquickjs.
pub trait Sealed {}

#[cfg(feature = "std")]
pub fn catch_unwind<R>(
    f: impl FnOnce() -> R + UnwindSafe,
) -> Result<R, alloc::boxed::Box<dyn core::any::Any + Send + 'static>> {
    std::panic::catch_unwind(f)
}

#[cfg(not(feature = "std"))]
pub fn catch_unwind<R>(
    f: impl FnOnce() -> R + UnwindSafe,
) -> Result<R, alloc::boxed::Box<dyn core::any::Any + Send + 'static>> {
    Ok(f())
}

#[cfg(feature = "std")]
pub fn resume_unwind(payload: alloc::boxed::Box<dyn core::any::Any + Send>) -> ! {
    std::panic::resume_unwind(payload)
}

#[cfg(not(feature = "std"))]
pub fn resume_unwind(_payload: alloc::boxed::Box<dyn core::any::Any + Send>) -> ! {
    panic!()
}
