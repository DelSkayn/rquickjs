use crate::{Mut, Ref, SendWhenParallel, Weak};
use cooked_waker::{IntoWaker, WakeRef};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};
use vec_arena::Arena;

#[cfg(feature = "parallel")]
use async_executor::Executor as AsyncExecutor;
#[cfg(not(feature = "parallel"))]
use async_executor::LocalExecutor as AsyncExecutor;

#[cfg(feature = "parallel")]
use futures_lite::future::Boxed as TaskFuture;
#[cfg(not(feature = "parallel"))]
use futures_lite::future::BoxedLocal as TaskFuture;

use futures_lite::FutureExt;

#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Executor(Weak<State>);

impl Executor {
    pub(crate) fn new() -> (Self, Spawner) {
        let state = Ref::new(Default::default());
        (Self(Ref::downgrade(&state)), Spawner(state))
    }
}

impl Future for Executor {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(state) = self.0.upgrade() {
            if state.executor.try_tick() {
                cx.waker().wake_by_ref();
            } else {
                state.set_waker(cx.waker().clone());
                state.idles.lock().retain(|_, waker| {
                    wake(waker.take());
                    false
                });
            }
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub struct Spawner(Ref<State>);

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future + SendWhenParallel + 'static,
    {
        let future = Box::pin(async move {
            future.await;
        });
        self.0.executor.spawn(Task::new(&self.0, future)).detach();
        wake(self.0.get_waker());
    }

    pub fn idle(&self) -> Idle {
        Idle::new(&self.0)
    }
}

#[inline]
fn wake(waker: Option<Waker>) {
    if let Some(waker) = waker {
        waker.wake();
    }
}

struct State {
    executor: AsyncExecutor<'static>,
    idles: Mut<Arena<Option<Waker>>>,
    waker: Mut<Option<Waker>>,
}

impl State {
    #[inline]
    fn set_waker(&self, waker: Waker) {
        *self.waker.lock() = Some(waker);
    }

    #[inline]
    fn get_waker(&self) -> Option<Waker> {
        self.waker.lock().take()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            executor: AsyncExecutor::new(),
            idles: Mut::new(Arena::new()),
            waker: Mut::new(None),
        }
    }
}

struct Task {
    future: TaskFuture<()>,
    state: Weak<State>,
}

impl Task {
    #[inline]
    fn new(state: &Ref<State>, future: TaskFuture<()>) -> Self {
        Task {
            state: Ref::downgrade(state),
            future,
        }
    }
}

impl Future for Task {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let waker = Arc::new(TaskWaker::new(&self.state, cx.waker().clone())).into_waker();
        let mut cx = Context::from_waker(&waker);
        self.as_mut().future.poll(&mut cx)
    }
}

struct TaskWaker {
    state: Weak<State>,
    waker: Mut<Option<Waker>>,
}

impl TaskWaker {
    #[inline]
    fn new(state: &Weak<State>, waker: Waker) -> Self {
        Self {
            state: state.clone(),
            waker: Mut::new(Some(waker)),
        }
    }

    #[inline]
    fn get_state(&self) -> Option<Ref<State>> {
        self.state.upgrade()
    }

    #[inline]
    fn get_waker(&self) -> Option<Waker> {
        self.waker.lock().take()
    }
}

unsafe impl Send for TaskWaker {}
unsafe impl Sync for TaskWaker {}

impl WakeRef for TaskWaker {
    fn wake_by_ref(&self) {
        wake(self.get_waker());
        if let Some(state) = self.get_state() {
            wake(state.get_waker());
        }
    }
}

/// The idle awaiting future
pub struct Idle(Option<IdleData>);

struct IdleData {
    id: usize,
    state: Weak<State>,
}

impl Idle {
    fn new(state: &Ref<State>) -> Self {
        let id = {
            if state.executor.is_empty() {
                return Self(None);
            }
            state.idles.lock().insert(None)
        };
        Self(Some(IdleData {
            id,
            state: Ref::downgrade(state),
        }))
    }
}

impl Drop for Idle {
    fn drop(&mut self) {
        if let Some(data) = &self.0 {
            if let Some(state) = data.state.upgrade() {
                state.idles.lock().remove(data.id);
            }
        }
    }
}

impl Future for Idle {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(data) = &self.0 {
            if let Some(state) = data.state.upgrade() {
                if state.executor.is_empty() {
                    state.idles.lock().remove(data.id);
                } else {
                    state.idles.lock()[data.id] = Some(cx.waker().clone());
                    return Poll::Pending;
                }
            }
        }
        Poll::Ready(())
    }
}
