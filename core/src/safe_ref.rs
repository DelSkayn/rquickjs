#[cfg(not(feature = "parallel"))]
use core::cell::RefCell as Cell;

#[cfg(feature = "parallel")]
use std::sync::Mutex as Cell;

#[cfg(not(feature = "parallel"))]
pub use core::cell::RefMut as Lock;

#[cfg(not(feature = "parallel"))]
pub use alloc::rc::{Rc as Ref, Weak};

#[cfg(feature = "parallel")]
pub use std::sync::{Arc as Ref, MutexGuard as Lock, Weak};

#[repr(transparent)]
pub struct Mut<T: ?Sized>(Cell<T>);

impl<T> Mut<T> {
    pub fn new(inner: T) -> Self {
        Self(Cell::new(inner))
    }
}

impl<T: Default> Default for Mut<T> {
    fn default() -> Self {
        Mut::new(T::default())
    }
}

impl<T: ?Sized> Mut<T> {
    pub fn lock(&self) -> Lock<T> {
        #[cfg(not(feature = "parallel"))]
        {
            self.0.borrow_mut()
        }

        #[cfg(feature = "parallel")]
        {
            // A Rust panic can unwind out of a `Context::with` closure while the
            // lock is held. QuickJS keeps its own exception state, so a poisoned
            // lock must not make the runtime unusable afterwards.
            self.0.lock().unwrap_or_else(|e| e.into_inner())
        }
    }

    pub fn try_lock(&self) -> Option<Lock<T>> {
        #[cfg(not(feature = "parallel"))]
        {
            self.0.try_borrow_mut().ok()
        }

        #[cfg(feature = "parallel")]
        {
            match self.0.try_lock() {
                Ok(guard) => Some(guard),
                Err(std::sync::TryLockError::Poisoned(e)) => Some(e.into_inner()),
                Err(std::sync::TryLockError::WouldBlock) => None,
            }
        }
    }
}
