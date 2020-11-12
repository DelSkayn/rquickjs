#[cfg(feature = "parallel")]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(not(feature = "parallel"))]
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

#[cfg(not(feature = "parallel"))]
pub(crate) type RefGuard<'a, T> = RefMut<'a, T>;

#[cfg(feature = "parallel")]
pub(crate) type RefGuard<'a, T> = MutexGuard<'a, T>;

pub(crate) struct Ref<T>(
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

    pub fn try_lock(&self) -> Option<RefGuard<T>> {
        Some(self.0.borrow_mut())
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

    pub fn try_lock(&self) -> Option<RefGuard<T>> {
        self.0.lock().ok()
    }
}
