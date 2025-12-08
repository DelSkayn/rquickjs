use alloc::boxed::Box;
use core::{
    future::Future,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{markers::ParallelSend, runtime::task_queue::TaskPoll, AsyncContext, Ctx};

pub struct WithFuture<'a, F, R> {
    context: &'a AsyncContext,
    state: WithFutureState<'a, F, R>,
}

enum WithFutureState<'a, F, R> {
    Initial {
        closure: F,
    },
    Running {
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
        let this = unsafe { self.get_unchecked_mut() };

        // Try to get lock - yields to executor if unavailable
        let Some(mut lock) = this.context.0.rt().try_lock() else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };

        lock.runtime.update_stack_top();

        // Create or get the future
        let mut future = match mem::replace(&mut this.state, WithFutureState::Done) {
            WithFutureState::Initial { closure } => {
                let ctx = unsafe { Ctx::new_async(this.context) };
                Box::pin(closure(ctx))
            }
            WithFutureState::Running { future } => future,
            WithFutureState::Done => panic!("WithFuture polled after completion"),
        };

        // Poll the future and spawned tasks
        loop {
            if let Poll::Ready(x) = future.as_mut().poll(cx) {
                return Poll::Ready(x);
            }

            let mut made_progress = false;

            if lock.runtime.get_opaque().poll(cx) == TaskPoll::Progress {
                made_progress = true;
            }

            loop {
                match lock.runtime.execute_pending_job() {
                    Ok(false) => break,
                    Ok(true) => made_progress = true,
                    Err(_) => made_progress = true,
                }
            }

            if !made_progress {
                this.state = WithFutureState::Running { future };
                return Poll::Pending;
            }
        }
    }
}

#[cfg(feature = "parallel")]
unsafe impl<F, R> Send for WithFuture<'_, F, R> {}
