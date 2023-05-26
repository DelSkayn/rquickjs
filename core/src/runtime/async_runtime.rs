use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::Runtime;

pub struct Pending<'js>(Vec<Pin<Box<dyn Future<Output = ()> + 'js>>>);

impl<'js> Pending<'js> {
    pub fn new() -> Self {
        Pending(Vec::new())
    }

    pub fn push<F: Future<Output = ()> + 'js>(&mut self, f: F) {
        self.0.push(Box::pin(f))
    }

    pub fn poll(&'js mut self) -> PendingFut {
        PendingFut(self)
    }
}

pub struct PendingFut<'js>(&'js mut Pending<'js>);

impl Future for PendingFut<'_> {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 .0.is_empty() {
            return Poll::Ready(false);
        }

        let item = self
            .0
             .0
            .iter_mut()
            .enumerate()
            .find_map(|(i, f)| match f.as_mut().poll(cx) {
                Poll::Ready(_) => Some(i),
                Poll::Pending => None,
            });
        match item {
            Some(idx) => {
                self.0 .0.swap_remove(idx);
                Poll::Ready(true)
            }
            None => Poll::Pending,
        }
    }
}

impl Runtime {
    /// Execute a single job
    pub async fn execute_job(&self) -> bool {
        let mut lock = self.inner.async_lock().await;
        if unsafe { lock.get_opaque_mut() }.pending.poll().await {
            return true;
        }

        if let Ok(true) = self.execute_pending_job() {
            return true;
        }
        false
    }

    /// Run until all futures and jobs in the runtime are finished.
    pub async fn idle(&self) {
        let mut lock = self.inner.async_lock().await;
        loop {
            if unsafe { lock.get_opaque_mut() }.pending.poll().await {
                continue;
            }

            if let Ok(false) = lock.execute_pending_job() {
                return;
            }
        }
    }
}
