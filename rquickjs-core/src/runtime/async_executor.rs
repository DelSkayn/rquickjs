use crate::{Ref, SendWhenParallel};
use async_task::Runnable;
use flume::{r#async::RecvStream, unbounded, Receiver, Sender};
use futures_lite::Stream;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};

#[cfg(feature = "parallel")]
use async_task::spawn as spawn_task;
#[cfg(not(feature = "parallel"))]
use async_task::spawn_local as spawn_task;

pin_project! {
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub struct Executor {
        #[pin]
        tasks: RecvStream<'static, Runnable>,
        idles: Receiver<Waker>,
        idle: Ref<AtomicBool>,
    }
}

impl Executor {
    pub(crate) fn new() -> (Self, Spawner) {
        let (tasks_tx, tasks_rx) = unbounded();
        let (idles_tx, idles_rx) = unbounded();
        let idle = Ref::new(AtomicBool::new(true));
        (
            Self {
                tasks: tasks_rx.into_stream(),
                idles: idles_rx,
                idle: idle.clone(),
            },
            Spawner {
                tasks: tasks_tx,
                idles: idles_tx,
                idle,
            },
        )
    }
}

impl Future for Executor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let result = {
            if let Poll::Ready(task) = self.as_mut().project().tasks.poll_next(cx) {
                if let Some(task) = task {
                    task.run();
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                } else {
                    // spawner is closed and queue is empty
                    Poll::Ready(())
                }
            } else {
                // spawner is alive and queue is empty
                Poll::Pending
            }
        };

        self.idle.store(true, Ordering::SeqCst);

        // wake idle futures
        while let Ok(waker) = self.idles.try_recv() {
            waker.wake();
        }

        result
    }
}

pub struct Spawner {
    tasks: Sender<Runnable>,
    idles: Sender<Waker>,
    idle: Ref<AtomicBool>,
}

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future + SendWhenParallel + 'static,
    {
        let (runnable, task) = spawn_task(
            async move {
                future.await;
            },
            self.schedule(),
        );
        task.detach();
        runnable.schedule();
    }

    fn schedule(&self) -> impl Fn(Runnable) + Send + Sync + 'static {
        let tasks = self.tasks.clone();
        move |runnable: Runnable| {
            tasks
                .send(runnable)
                .expect("Async executor unfortunately destroyed");
        }
    }

    pub fn idle(&self) -> Idle {
        if self.idle.load(Ordering::SeqCst) {
            Idle::default()
        } else {
            Idle::new(&self.idles)
        }
    }
}

/// The idle awaiting future
#[derive(Default)]
pub struct Idle(Option<Sender<Waker>>);

impl Idle {
    fn new(sender: &Sender<Waker>) -> Self {
        Self(Some(sender.clone()))
    }
}

impl Future for Idle {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(sender) = &self.0 {
            if sender.send(cx.waker().clone()).is_ok() {
                return Poll::Pending;
            }
        }
        Poll::Ready(())
    }
}
