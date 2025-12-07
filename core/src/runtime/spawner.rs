use super::{task_queue::TaskPoll, AsyncWeakRuntime};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Future that drives the runtime's spawned tasks in the background.
pub struct DriveFuture {
    rt: AsyncWeakRuntime,
}

#[cfg(feature = "parallel")]
unsafe impl Send for DriveFuture {}
#[cfg(feature = "parallel")]
unsafe impl Sync for DriveFuture {}

impl DriveFuture {
    pub(crate) fn new(rt: AsyncWeakRuntime) -> Self {
        Self { rt }
    }
}

impl Future for DriveFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(runtime) = self.rt.try_ref() else {
            return Poll::Ready(());
        };

        let Some(mut lock) = runtime.try_lock() else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };

        lock.runtime.update_stack_top();
        lock.runtime.get_opaque().listen(cx.waker().clone());

        loop {
            if let Ok(true) = lock.runtime.execute_pending_job() {
                continue;
            }
            match lock.runtime.get_opaque().poll(cx) {
                TaskPoll::Progress => continue,
                _ => break,
            }
        }

        Poll::Pending
    }
}
