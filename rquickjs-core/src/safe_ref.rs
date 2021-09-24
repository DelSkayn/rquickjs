#[cfg(not(feature = "parallel"))]
use std::cell::RefCell as Cell;

#[cfg(feature = "parallel")]
use std::sync::Mutex as Cell;

#[cfg(not(feature = "parallel"))]
pub use std::{
    cell::RefMut as Lock,
    rc::{Rc as Ref, Weak},
};

#[cfg(feature = "parallel")]
pub use std::sync::{Arc as Ref, MutexGuard as Lock, Weak};

use std::sync::PoisonError;

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
            self.0.lock().unwrap()
        }
    }

    /// Lock the 'mutex' and return the value if the lock is poisoned
    pub fn try_lock(&self) -> Result<Lock<T>, PoisonError<Lock<T>>> {
        #[cfg(not(feature = "parallel"))]
        {
            Ok(self.lock())
        }
        #[cfg(feature = "parallel")]
        {
            match self.0.lock() {
                Ok(x) => Ok(x),
                Err(x) => Err(x),
            }
        }
    }
}
