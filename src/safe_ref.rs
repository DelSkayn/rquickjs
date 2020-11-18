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
pub struct Ref<T>(
    #[cfg(not(feature = "parallel"))] Rc<RefCell<T>>,
    #[cfg(feature = "parallel")] Arc<Mutex<T>>,
);

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(not(feature = "parallel"))]
impl<T> Ref<T> {
    pub fn new(inner: T) -> Self {
        Self(Rc::new(RefCell::new(inner)))
    }

    pub fn lock(&self) -> RefGuard<T> {
        self.0.borrow_mut()
    }

    pub fn try_lock(&self) -> Result<RefGuard<T>, RefGuard<T>> {
        Ok(self.0.borrow_mut())
    }

    pub fn weak(&self) -> WeakRef<T> {
        WeakRef(Rc::downgrade(&self.0))
    }
}

#[cfg(feature = "parallel")]
impl<T> Ref<T> {
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

    pub fn weak(&self) -> WeakRef<T> {
        WeakRef(Arc::downgrade(&self.0))
    }
}

#[repr(transparent)]
pub struct WeakRef<T>(
    #[cfg(not(feature = "parallel"))] Weak<RefCell<T>>,
    #[cfg(feature = "parallel")] Weak<Mutex<T>>,
);

impl<T> Clone for WeakRef<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> WeakRef<T> {
    pub fn try_ref(&self) -> Option<Ref<T>> {
        self.0.upgrade().map(Ref)
    }
}
