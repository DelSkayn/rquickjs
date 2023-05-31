use std::{future::Future, pin::Pin, task::Poll};

/// A structure to hold futures spawned inside the runtime.
///
/// TODO: change future lookup in poll from O(n) to O(1).
pub struct Spawner<'js> {
    futures: Vec<Pin<Box<dyn Future<Output = ()> + 'js>>>,
}

impl<'js> Spawner<'js> {
    pub fn new() -> Self {
        Spawner {
            futures: Vec::new(),
        }
    }

    pub fn push<F>(&mut self, f: F)
    where
        F: Future<Output = ()> + 'js,
    {
        self.futures.push(Box::pin(f))
    }

    pub fn drive<'a>(&'a mut self) -> SpawnFuture<'a, 'js> {
        SpawnFuture(self)
    }

    pub fn is_empty(&mut self) -> bool {
        self.futures.is_empty()
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
