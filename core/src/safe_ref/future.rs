pub use std::sync::{Arc as Ref, Weak};
#[cfg(feature = "futures")]
use tokio_rs::sync::{Mutex, MutexGuard};

#[repr(transparent)]
pub struct Mut<T: ?Sized>(Mutex<T>);

pub type Lock<'a, T> = MutexGuard<'a, T>;

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
        self.0.blocking_lock()
    }

    pub fn try_lock(&self) -> Option<Lock<T>> {
        self.0.try_lock().ok()
    }

    pub async fn async_lock(&self) -> Lock<T> {
        self.0.lock().await
    }
}
