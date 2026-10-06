use super::{
    schedular::{Schedular, SchedularPoll},
    AsyncWeakRuntime, InnerRuntime,
};
use crate::{AsyncRuntime, Ctx};
use alloc::vec::Vec;
use core::{
    future::Future,
    pin::Pin,
    ptr::NonNull,
    task::{ready, Context, Poll, Waker},
};

use async_lock::futures::LockArc;

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

    pub fn is_empty(&mut self) -> bool {
        self.schedular.is_empty()
    }

    pub fn poll(&mut self, cx: &mut Context) -> SchedularPoll {
        unsafe { self.schedular.poll(cx) }
    }
}

enum DriveFutureState {
    Initial,
    Lock {
        lock_future: Option<LockArc<InnerRuntime>>,
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

    fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        // Safety: We manually ensure that pinned values remained properly pinned.
        let this = unsafe { self.get_unchecked_mut() };
        loop {
            let mut lock = match this.state {
                DriveFutureState::Initial => {
                    let Some(_runtime) = this.rt.try_ref() else {
                        return Poll::Ready(());
                    };

                    let lock_future = _runtime.inner.lock_arc();
                    this.state = DriveFutureState::Lock {
                        lock_future: Some(lock_future),
                        _runtime,
                    };
                    continue;
                }
                DriveFutureState::Lock {
                    ref mut lock_future,
                    ..
                } => {
                    // Safety: The future will not be moved until it is ready and then dropped.
                    let res = unsafe {
                        ready!(Pin::new_unchecked(lock_future.as_mut().unwrap()).poll(cx))
                    };
                    // Assign none explicitly so it we don't move out of the future.
                    *lock_future = None;
                    res
                }
            };

            lock.runtime.update_stack_top();

            lock.runtime.get_opaque().listen(cx.waker().clone());

            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(ctx) => {
                        // Retain the failed job's exception; see
                        // `Opaque::job_error`. The drain continues after the
                        // failure.
                        //
                        // Safety: the runtime lock is held for the whole
                        // poll, so viewing the failed job's context is sound;
                        // `catch` clears the slot, making the retained value
                        // the failure's only copy.
                        let job_ctx = unsafe {
                            Ctx::from_raw(
                                NonNull::new(ctx).expect("QuickJS returned null ptr for job error"),
                            )
                        };
                        lock.runtime.get_opaque().retain_job_error(job_ctx.catch());
                    }
                }

                match lock.runtime.get_opaque().poll(cx) {
                    SchedularPoll::ShouldYield | SchedularPoll::Empty | SchedularPoll::Pending => {
                        break
                    }
                    SchedularPoll::PendingProgress => {}
                }
            }

            this.state = DriveFutureState::Initial;
            return Poll::Pending;
        }
    }
}
