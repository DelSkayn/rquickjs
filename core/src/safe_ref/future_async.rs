use async_std::sync::{Mutex, MutexGuard as Lock};
pub use std::sync::{Arc as Ref, Weak};

#[repr(transparent)]
pub struct Mut<T: ?Sized>(Mutex<T>);

impl<T> Mut<T> {
    pub fn new(inner: T) -> Self {
        Self(Mutex::new(inner))
    }
}

impl<T: Default> Default for Mut<T> {
    fn default() -> Self {
        Mut::new(T::default())
    }
}

impl<T: ?Sized> Mut<T> {
    pub fn lock(&self) -> Lock<T> {
        async_std::task::block_on(self.0.lock())
    }

    pub fn try_lock(&self) -> Option<Lock<T>> {
        self.0.try_lock()
    }

    pub async fn async_lock(&self) -> Lock<T> {
        self.0.lock().await
    }
}
