use std::{
    future::Future,
    pin::{pin, Pin},
    task::ready,
    task::{Poll, Waker},
};

use async_lock::futures::LockArc;

use crate::AsyncRuntime;

use super::{AsyncWeakRuntime, InnerRuntime};

/// A structure to hold futures spawned inside the runtime.
///
/// TODO: change future lookup in poll from O(n) to O(1).
pub struct Spawner<'js> {
    futures: Vec<Pin<Box<dyn Future<Output = ()> + 'js>>>,
    wakeup: Vec<Waker>,
}

impl<'js> Spawner<'js> {
    pub fn new() -> Self {
        Spawner {
            futures: Vec::new(),
            wakeup: Vec::new(),
        }
    }

    pub fn push<F>(&mut self, f: F)
    where
        F: Future<Output = ()> + 'js,
    {
        self.wakeup.drain(..).for_each(Waker::wake);
        self.futures.push(Box::pin(f))
    }

    pub fn listen(&mut self, wake: Waker) {
        self.wakeup.push(wake);
    }

    // Drives the runtime futures forward, returns false if their where no futures
    pub fn drive<'a>(&'a mut self) -> SpawnFuture<'a, 'js> {
        SpawnFuture(self)
    }

    pub fn is_empty(&mut self) -> bool {
        self.futures.is_empty()
    }
}

impl Drop for Spawner<'_> {
    fn drop(&mut self) {
        self.wakeup.drain(..).for_each(Waker::wake)
    }
}

pub struct SpawnFuture<'a, 'js>(&'a mut Spawner<'js>);

impl<'a, 'js> Future for SpawnFuture<'a, 'js> {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.0.futures.is_empty() {
            return Poll::Ready(false);
        }

        let mut completed_indices = Vec::new();

        // Iterate over the futures and check their completion status.
        for (i, f) in self.0.futures.iter_mut().enumerate() {
            if let Poll::Ready(_) = f.as_mut().poll(cx) {
                completed_indices.push(i);
            }
        }

        // Remove the completed futures in reverse order to avoid index shifting.
        for &idx in completed_indices.iter().rev() {
            self.0.futures.remove(idx);
        }

        if !completed_indices.is_empty() {
            Poll::Ready(true)
        } else {
            Poll::Pending
        }
    }
}

enum DriveFutureState {
    Initial,
    Lock {
        lock_future: LockArc<InnerRuntime>,
        // Here to ensure the lock remains valid.
        _runtime: AsyncRuntime,
    },
}

pub struct DriveFuture {
    rt: AsyncWeakRuntime,
    state: DriveFutureState,
}

#[cfg(feature = "parallel")]
unsafe impl Send for DriveFuture {}
#[cfg(feature = "parallel")]
unsafe impl Sync for DriveFuture {}

impl DriveFuture {
    pub(crate) fn new(rt: AsyncWeakRuntime) -> Self {
        Self {
            rt,
            state: DriveFutureState::Initial,
        }
    }
}

impl Future for DriveFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut lock = match self.state {
                DriveFutureState::Initial => {
                    let Some(_runtime) = self.rt.try_ref() else {
                        return Poll::Ready(());
                    };

                    let lock_future = _runtime.inner.lock_arc();
                    self.state = DriveFutureState::Lock {
                        lock_future,
                        _runtime,
                    };
                    continue;
                }
                DriveFutureState::Lock {
                    ref mut lock_future,
                    ..
                } => {
                    ready!(Pin::new(lock_future).poll(cx))
                }
            };

            lock.runtime.update_stack_top();

            unsafe { lock.runtime.get_opaque_mut() }
                .spawner()
                .listen(cx.waker().clone());

            loop {
                // TODO: Handle error.
                if let Ok(true) = lock.runtime.execute_pending_job() {
                    continue;
                }

                let drive = pin!(unsafe { lock.runtime.get_opaque_mut() }.spawner().drive());

                // TODO: Handle error.
                match drive.poll(cx) {
                    Poll::Pending => {
                        // Execute pending jobs to ensure we don't dead lock when waiting on
                        // QuickJS futures.
                        while let Ok(true) = lock.runtime.execute_pending_job() {}
                        self.state = DriveFutureState::Initial;
                        return Poll::Pending;
                    }
                    Poll::Ready(false) => {}
                    Poll::Ready(true) => {
                        continue;
                    }
                }

                break;
            }

            self.state = DriveFutureState::Initial;
            return Poll::Pending;
        }
    }
}
