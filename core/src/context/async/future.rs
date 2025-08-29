use alloc::boxed::Box;
use core::{
    future::Future,
    mem::{self, ManuallyDrop},
    pin::Pin,
    task::{ready, Context, Poll},
};

use async_lock::futures::Lock;

use crate::{
    markers::ParallelSend,
    runtime::{schedular::SchedularPoll, InnerRuntime},
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
        #[cfg(not(feature = "parallel"))]
        future: Pin<Box<dyn Future<Output = R> + 'a>>,
        #[cfg(feature = "parallel")]
        future: Pin<Box<dyn Future<Output = R> + 'a + Send>>,
    },
    Done,
}

#[cfg(not(feature = "parallel"))]
pub type CallbackFuture<'js, R> = Pin<Box<dyn Future<Output = R> + 'js>>;
#[cfg(feature = "parallel")]
pub type CallbackFuture<'js, R> = Pin<Box<dyn Future<Output = R> + 'js + Send>>;    

impl<'a, F, R> WithFuture<'a, F, R>
where
    F: for<'js> FnOnce(Ctx<'js>) -> CallbackFuture<'js, R> + ParallelSend,
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
    F: for<'js> FnOnce(Ctx<'js>) -> CallbackFuture<'js, R> + ParallelSend,
    R: ParallelSend + 'static,
{
    type Output = R;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Implementation ensures we don't break pin guarantees.
        let this = unsafe { self.get_unchecked_mut() };

        let mut lock = loop {
            // We don't move the lock_state as long as it is pending
            if let LockState::Pending(ref mut fut) = &mut this.lock_state {
                // SAFETY: Sound as we don't move future while it is pending.
                let pin = unsafe { Pin::new_unchecked(&mut **fut) };
                let lock = ready!(pin.poll(cx));
                // at this point we have acquired a lock, so we will now drop the future allowing
                // us to reused the memory space.
                unsafe { ManuallyDrop::drop(fut) };
                // The pinned memory is dropped so now we can freely move into it.
                this.lock_state = LockState::Initial;
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
                break Poll::Ready(x);
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

            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(false) => break,
                    Ok(true) => made_progress = true,
                    Err(_ctx) => {
                        // TODO figure out what to do with a job error.
                        made_progress = true;
                    }
                }
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
