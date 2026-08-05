#[cfg(not(feature = "parallel"))]
use core::cell::RefCell as Cell;

#[cfg(feature = "parallel")]
use std::sync::Mutex as Cell;

#[cfg(not(feature = "parallel"))]
pub use core::cell::RefMut as Lock;

#[cfg(not(feature = "parallel"))]
pub use alloc::rc::{Rc as Ref, Weak};

#[cfg(feature = "parallel")]
pub use std::sync::{Arc as Ref, Weak};

#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "parallel")]
std::thread_local! {
    /// Ids of the locks held by *this* thread. Being thread-local, a stale
    /// entry (from a leaked guard) can never be observed by another thread,
    /// and ids are process-unique so a reused heap address cannot alias a
    /// previous `Mut`.
    static HELD: core::cell::RefCell<alloc::vec::Vec<u64>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };
}

#[cfg(feature = "parallel")]
fn next_lock_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

pub struct Mut<T: ?Sized> {
    #[cfg(feature = "parallel")]
    id: u64,
    data: Cell<T>,
}

impl<T> Mut<T> {
    pub fn new(inner: T) -> Self {
        Self {
            #[cfg(feature = "parallel")]
            id: next_lock_id(),
            data: Cell::new(inner),
        }
    }
}

impl<T: Default> Default for Mut<T> {
    fn default() -> Self {
        Mut::new(T::default())
    }
}

#[cfg(feature = "parallel")]
pub struct Lock<'a, T: ?Sized> {
    id: u64,
    guard: std::sync::MutexGuard<'a, T>,
}

#[cfg(feature = "parallel")]
impl<T: ?Sized> core::ops::Deref for Lock<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

#[cfg(feature = "parallel")]
impl<T: ?Sized> core::ops::DerefMut for Lock<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

#[cfg(feature = "parallel")]
impl<T: ?Sized> Drop for Lock<'_, T> {
    fn drop(&mut self) {
        let id = self.id;
        let _ = HELD.try_with(|held| {
            let mut held = held.borrow_mut();
            if let Some(i) = held.iter().rposition(|&h| h == id) {
                held.remove(i);
            }
        });
    }
}

impl<T: ?Sized> Mut<T> {
    pub fn lock(&self) -> Lock<T> {
        #[cfg(not(feature = "parallel"))]
        {
            self.data.borrow_mut()
        }

        #[cfg(feature = "parallel")]
        {
            let guard = self.data.lock().unwrap();
            self.mark_held();
            Lock { id: self.id, guard }
        }
    }

    pub fn try_lock(&self) -> Option<Lock<T>> {
        #[cfg(not(feature = "parallel"))]
        {
            self.data.try_borrow_mut().ok()
        }

        #[cfg(feature = "parallel")]
        {
            let guard = self.data.lock().ok()?;
            self.mark_held();
            Some(Lock { id: self.id, guard })
        }
    }

    #[cfg(feature = "parallel")]
    fn mark_held(&self) {
        let _ = HELD.try_with(|held| held.borrow_mut().push(self.id));
    }

    /// Whether this thread already holds the lock further up its own call
    /// stack. `lock`/`try_lock` block, so callers that may run re-entrantly
    /// must check this first to avoid deadlocking on themselves.
    #[cfg(feature = "parallel")]
    pub fn is_locked_by_current_thread(&self) -> bool {
        HELD.try_with(|held| held.borrow().contains(&self.id))
            .unwrap_or(false)
    }
}
