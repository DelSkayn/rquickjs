use alloc::boxed::Box;
use core::{
    future::Future,
    mem::{self, ManuallyDrop},
    pin::Pin,
    result::Result as StdResult,
    task::{ready, Context, Poll},
};

use async_lock::futures::Lock;

use crate::{
    markers::ParallelSend,
    runtime::{job_error::JobError, schedular::SchedularPoll, InnerRuntime},
    AsyncContext, Ctx,
};

pub struct WithFuture<'a, F, R> {
    context: &'a AsyncContext,
    lock_state: LockState<'a>,
    state: WithFutureState<'a, F, R>,
}

enum LockState<'a> {
    Initial,
    Pending(ManuallyDrop<Lock<'a, InnerRuntime>>),
}

impl<'a> Drop for LockState<'a> {
    fn drop(&mut self) {
        if let LockState::Pending(ref mut x) = self {
            unsafe { ManuallyDrop::drop(x) }
        }
    }
}

enum WithFutureState<'a, F, R> {
    Initial {
        closure: F,
    },
    FutureCreated {
        future: Pin<Box<dyn Future<Output = R> + 'a>>,
    },
    Done,
}

impl<'a, F, R> WithFuture<'a, F, R>
where
    F: for<'js> AsyncFnOnce(Ctx<'js>) -> R + ParallelSend,
    R: ParallelSend,
{
    pub fn new(context: &'a AsyncContext, f: F) -> Self {
        Self {
            context,
            lock_state: LockState::Initial,
            state: WithFutureState::Initial { closure: f },
        }
    }
}

impl<'a, F, R> Future for WithFuture<'a, F, R>
where
    F: for<'js> AsyncFnOnce(Ctx<'js>) -> R + ParallelSend + 'a,
    R: ParallelSend + 'static,
{
    type Output = StdResult<R, JobError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Implementation ensures we don't break pin guarantees.
        let this = unsafe { self.get_unchecked_mut() };

        let lock = loop {
            // We don't move the lock_state as long as it is pending
            if let LockState::Pending(ref mut fut) = &mut this.lock_state {
                // SAFETY: Sound as we don't move future while it is pending.
                let pin = unsafe { Pin::new_unchecked(&mut **fut) };
                let lock = ready!(pin.poll(cx));
                // Replace lock_state with Initial first, so the old Pending variant
                // is moved out and its Drop runs exactly once.
                let old = mem::replace(&mut this.lock_state, LockState::Initial);
                drop(old);
                break lock;
            } else {
                // we assign a state with manually drop so we can drop the value when we need to
                // replace it.
                // Assign
                this.lock_state =
                    LockState::Pending(ManuallyDrop::new(this.context.0.rt().inner.lock()));
            }
        };

        lock.runtime.update_stack_top();
        unsafe { lock.runtime.get_opaque().drain_job_errors() };

        // At this point we have locked the runtime so we start running the actual future
        // we can move this memory since the future is boxed and thus movable.
        let mut future = match mem::replace(&mut this.state, WithFutureState::Done) {
            WithFutureState::Initial { closure } => {
                // SAFETY: we have a lock, so creating this ctx is save.
                let ctx = unsafe { Ctx::new_async(this.context) };
                Box::pin(closure(ctx))
            }
            WithFutureState::FutureCreated { future } => future,
            // The future was called an additional time,
            // We don't have anything valid to do here so just panic.
            WithFutureState::Done => panic!("With future called after it returned"),
        };

        let res = loop {
            let mut made_progress = false;

            if let Poll::Ready(x) = future.as_mut().poll(cx) {
                drop(future);
                break Poll::Ready(Ok(x));
            };

            let opaque = lock.runtime.get_opaque();
            match opaque.poll(cx) {
                SchedularPoll::Empty => {
                    // if the schedular is empty that means the future is waiting on an external or
                    // on a promise.
                }
                SchedularPoll::ShouldYield => {
                    this.state = WithFutureState::FutureCreated { future };
                    return Poll::Pending;
                }
                SchedularPoll::Pending => {
                    // we couldn't drive any futures so we should run some jobs to see we can get
                    // some progress.
                }
                SchedularPoll::PendingProgress => {
                    // We did make some progress so the root future might not be blocked, but it is
                    // probably still a good idea to run some jobs as most futures first require a
                    // single job to run before unblocking.
                    made_progress = true;
                }
            };

            let mut job_error = None;
            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(false) => break,
                    Ok(true) => made_progress = true,
                    Err(err) => {
                        job_error = Some(err);
                        break;
                    }
                }
            }

            // The failed job may have been the one that would have settled the
            // promise the closure awaits, so cancel the closure and report it.
            if let Some(err) = job_error {
                // Drop the closure while the runtime lock is still held: it may
                // own JavaScript values that must be freed under the lock.
                drop(future);
                break Poll::Ready(Err(err));
            }

            // If no work could be done we should yield back.
            if !made_progress {
                this.state = WithFutureState::FutureCreated { future };
                break Poll::Pending;
            }
        };

        // Manually drop the lock so it isn't accidentally moved into somewhere.
        mem::drop(lock);

        res
    }
}

#[cfg(feature = "parallel")]
unsafe impl<F, R> Send for WithFuture<'_, F, R> {}
