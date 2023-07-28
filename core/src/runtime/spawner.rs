use std::{
    future::Future,
    pin::{pin, Pin},
    task::ready,
    task::{Poll, Waker},
};

use async_lock::futures::LockArc;

use crate::AsyncRuntime;

use super::{raw::RawRuntime, AsyncWeakRuntime};

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

        let item =
            self.0
                .futures
                .iter_mut()
                .enumerate()
                .find_map(|(i, f)| match f.as_mut().poll(cx) {
                    Poll::Ready(_) => Some(i),
                    Poll::Pending => None,
                });

        match item {
            Some(idx) => {
                self.0.futures.swap_remove(idx);
                Poll::Ready(true)
            }
            None => Poll::Pending,
        }
    }
}

enum DriveFutureState {
    Initial,
    Lock {
        lock_future: LockArc<RawRuntime>,
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

                    // Dirty hack to get a owned lock,
                    // We know the lock will remain alive and won't be moved since it is inside a
                    // arc like structure and we keep it alive in the lock.
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

            lock.update_stack_top();

            unsafe { lock.get_opaque_mut() }
                .spawner()
                .listen(cx.waker().clone());

            loop {
                // TODO: Handle error.
                if let Ok(true) = lock.execute_pending_job() {
                    continue;
                }

                let drive = pin!(unsafe { lock.get_opaque_mut() }.spawner().drive());

                // TODO: Handle error.
                match drive.poll(cx) {
                    Poll::Pending => {
                        // Execute pending jobs to ensure we don't dead lock when waiting on
                        // quickjs futures.
                        while let Ok(true) = lock.execute_pending_job() {}
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
