use std::{
    future::Future,
    pin::{pin, Pin},
    task::{ready, Poll, Waker},
};

use async_lock::futures::LockArc;

use crate::AsyncRuntime;

use super::{schedular::Schedular, AsyncWeakRuntime, InnerRuntime};

/// A structure to hold futures spawned inside the runtime.
pub struct Spawner {
    schedular: Schedular,
    wakeup: Vec<Waker>,
}

impl Spawner {
    pub fn new() -> Self {
        Spawner {
            schedular: Schedular::new(),
            wakeup: Vec::new(),
        }
    }

    pub unsafe fn push<F>(&mut self, f: F)
    where
        F: Future<Output = ()>,
    {
        unsafe { self.schedular.push(f) };
        self.wakeup.drain(..).for_each(Waker::wake);
    }

    pub fn listen(&mut self, wake: Waker) {
        self.wakeup.push(wake);
    }

    // Drives the runtime futures forward, returns false if their where no futures
    pub fn drive<'a>(&'a self) -> SpawnFuture<'a> {
        SpawnFuture(self)
    }

    pub fn is_empty(&mut self) -> bool {
        self.schedular.is_empty()
    }
}

pub struct SpawnFuture<'a>(&'a Spawner);

impl<'a> Future for SpawnFuture<'a> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        unsafe { self.0.schedular.poll(cx) }
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
