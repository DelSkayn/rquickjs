use std::{
    future::Future,
    mem::{self, ManuallyDrop},
    pin::{pin, Pin},
    task::{ready, Context, Poll},
};

use async_lock::futures::Lock;

use crate::{markers::ParallelSend, runtime::InnerRuntime, AsyncContext, Ctx};

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
        future: Pin<Box<dyn Future<Output = R> + 'a + Send>>,
    },
    Done,
}

impl<'a, F, R> WithFuture<'a, F, R>
where
    F: for<'js> FnOnce(Ctx<'js>) -> Pin<Box<dyn Future<Output = R> + 'js + Send>> + ParallelSend,
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
    F: for<'js> FnOnce(Ctx<'js>) -> Pin<Box<dyn Future<Output = R> + 'js + Send>> + ParallelSend,
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
                    LockState::Pending(ManuallyDrop::new(this.context.0.rt.inner.lock()));
            }
        };

        lock.runtime.update_stack_top();

        // At this point we have locked the runtime so we start running the actual future
        let res = loop {
            // we can move this memory since the future is boxed and thus movable.
            match mem::replace(&mut this.state, WithFutureState::Done) {
                WithFutureState::Initial { closure } => {
                    // SAFETY: we have a lock, so creating this ctx is save.
                    let ctx = unsafe { Ctx::new_async(this.context) };
                    let future = Box::pin(closure(ctx));
                    this.state = WithFutureState::FutureCreated { future };
                }
                WithFutureState::FutureCreated { mut future } => match future.as_mut().poll(cx) {
                    Poll::Ready(x) => {
                        break Poll::Ready(x);
                    }
                    Poll::Pending => {
                        // put the future back
                        this.state = WithFutureState::FutureCreated { future };
                        break Poll::Pending;
                    }
                },
                // The future was called an additional time,
                // We don't have anything valid to do here so just panic.
                WithFutureState::Done => panic!("With future called after it returned"),
            }
        };

        // the future was pending so it is possibly waiting on some JavaScript job, so drive the
        // JavaScript runtime for as long as possible.
        if res.is_pending() {
            loop {
                // drive the futures stored in the runtime.
                let future = unsafe { lock.runtime.get_opaque_mut() }.spawner().drive();
                let future = pin!(future);
                match future.poll(cx) {
                    // A future completed, try again in case any other future can complete.
                    Poll::Ready(true) => continue,
                    Poll::Ready(false) | Poll::Pending => {}
                }

                lock.runtime.is_job_pending();
                match lock.runtime.execute_pending_job() {
                    Ok(false) => break,
                    Ok(true) => {}
                    Err(_ctx) => {
                        //TODO figure out what to do with job errors.
                    }
                }
            }
        }

        // Manually drop the lock so it isn't accidentally moved into somewhere.
        mem::drop(lock);

        res
    }
}

#[cfg(feature = "parallel")]
unsafe impl<F, R> Send for WithFuture<'_, F, R> {}
