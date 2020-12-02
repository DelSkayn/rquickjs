#[cfg(feature = "parallel")]
use std::sync::{Arc, Mutex, Weak};
#[cfg(not(feature = "parallel"))]
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[cfg(not(feature = "parallel"))]
pub use std::cell::RefMut as RefGuard;
#[cfg(feature = "parallel")]
pub use std::sync::MutexGuard as RefGuard;

#[repr(transparent)]
pub struct SafeRef<T>(
    #[cfg(not(feature = "parallel"))] Rc<RefCell<T>>,
    #[cfg(feature = "parallel")] Arc<Mutex<T>>,
);

impl<T> Clone for SafeRef<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(not(feature = "parallel"))]
impl<T> SafeRef<T> {
    pub fn new(inner: T) -> Self {
        Self(Rc::new(RefCell::new(inner)))
    }

    pub fn lock(&self) -> RefGuard<T> {
        self.0.borrow_mut()
    }

    pub fn try_lock(&self) -> Result<RefGuard<T>, RefGuard<T>> {
        Ok(self.0.borrow_mut())
    }

    pub fn weak(&self) -> SafeWeakRef<T> {
        SafeWeakRef(Rc::downgrade(&self.0))
    }
}

#[cfg(feature = "parallel")]
impl<T> SafeRef<T> {
    pub fn new(inner: T) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn lock(&self) -> RefGuard<T> {
        self.0.lock().unwrap()
    }

    pub fn try_lock(&self) -> Result<RefGuard<T>, RefGuard<T>> {
        match self.0.lock() {
            Ok(x) => Ok(x),
            Err(x) => Err(x.into_inner()),
        }
    }

    pub fn weak(&self) -> SafeWeakRef<T> {
        SafeWeakRef(Arc::downgrade(&self.0))
    }
}

#[repr(transparent)]
pub struct SafeWeakRef<T>(
    #[cfg(not(feature = "parallel"))] Weak<RefCell<T>>,
    #[cfg(feature = "parallel")] Weak<Mutex<T>>,
);

impl<T> Clone for SafeWeakRef<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> SafeWeakRef<T> {
    pub fn try_ref(&self) -> Option<SafeRef<T>> {
        self.0.upgrade().map(SafeRef)
    }
}
