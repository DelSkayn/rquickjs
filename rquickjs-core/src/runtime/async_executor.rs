use crate::{Mut, Ref, SendWhenParallel, Weak};
use cooked_waker::{IntoWaker, WakeRef};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};
use vec_arena::Arena;

#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub struct Executor(Weak<Mut<State>>);

impl Executor {
    pub fn new() -> (Self, Spawner) {
        let state = Ref::new(Mut::new(Default::default()));
        (Self(Ref::downgrade(&state)), Spawner(state))
    }
}

impl Future for Executor {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(state) = self.0.upgrade() {
            loop {
                if let Some((id, mut future)) = {
                    let mut state = state.lock();
                    state.dequeue()
                } {
                    let waker = Arc::new(TaskWaker {
                        id,
                        state: state.clone(),
                    })
                    .into_waker();
                    let mut cx = Context::from_waker(&waker);

                    if let Poll::Pending = future.as_mut().poll(&mut cx) {
                        let mut state = state.lock();
                        state.respawn(id, future);
                    } else {
                        let mut state = state.lock();
                        state.unspawn(id);
                    }
                } else {
                    break;
                }
            }
            {
                let mut state = state.lock();
                state.postprocess(cx);
            }
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub struct Spawner(Ref<Mut<State>>);

impl Drop for Spawner {
    fn drop(&mut self) {
        wake({
            let mut state = self.0.lock();
            state.finalize()
        });
    }
}

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future + SendWhenParallel + 'static,
    {
        let future = Box::pin(async move {
            future.await;
        });
        wake({
            let mut state = self.0.lock();
            state.spawn(future)
        });
    }

    pub fn idle(&self) -> Idle {
        Idle::new(&self.0)
    }
}

fn wake(waker: Option<Waker>) {
    if let Some(waker) = waker {
        waker.wake();
    }
}

#[cfg(not(feature = "parallel"))]
type TaskFuture = Pin<Box<dyn Future<Output = ()>>>;

#[cfg(feature = "parallel")]
type TaskFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

const NULL: usize = !0;

struct Task {
    next: usize,
    future: Option<TaskFuture>,
}

struct State {
    tasks: Arena<Task>,
    idles: Arena<Option<Waker>>,
    first: usize,
    last: usize,
    waker: Option<Waker>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tasks: Arena::new(),
            idles: Arena::new(),
            first: NULL,
            last: NULL,
            waker: None,
        }
    }
}

impl State {
    fn enqueue(&mut self, id: usize) {
        if self.first == NULL {
            // empty queue
            self.first = id;
        } else {
            // non-empty queue
            let task = &mut self.tasks[self.last];
            if task.next != NULL || self.last == id {
                // already queued
                return;
            }
            task.next = id;
        }
        self.last = id;
    }

    fn spawn(&mut self, future: TaskFuture) -> Option<Waker> {
        let task = Task {
            next: NULL,
            future: Some(future),
        };
        let id = self.tasks.insert(task);
        self.schedule(id)
    }

    fn schedule(&mut self, id: usize) -> Option<Waker> {
        self.enqueue(id);
        self.waker.take()
    }

    fn finalize(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    fn dequeue(&mut self) -> Option<(usize, TaskFuture)> {
        if self.first == NULL {
            None
        } else {
            let id = self.first;
            let task = &mut self.tasks[id];
            self.first = task.next;
            if self.first == NULL {
                self.last = NULL;
            } else {
                task.next = NULL;
            }
            task.future.take().map(|future| (id, future))
        }
    }

    fn respawn(&mut self, id: usize, future: TaskFuture) {
        let task = &mut self.tasks[id];
        task.future = Some(future);
    }

    fn unspawn(&mut self, id: usize) {
        self.tasks.remove(id);
    }

    fn postprocess(&mut self, cx: &mut Context) {
        self.waker = Some(cx.waker().clone());
        if self.tasks.is_empty() && !self.idles.is_empty() {
            self.idles.retain(|_, waker| {
                if let Some(waker) = waker.take() {
                    waker.wake();
                }
                false
            })
        }
    }
}

struct TaskWaker {
    id: usize,
    state: Ref<Mut<State>>,
}

unsafe impl Send for TaskWaker {}
unsafe impl Sync for TaskWaker {}

impl WakeRef for TaskWaker {
    fn wake_by_ref(&self) {
        wake({
            let mut state = self.state.lock();
            state.schedule(self.id)
        });
    }
}

pub struct Idle(Option<IdleData>);

struct IdleData {
    id: usize,
    state: Weak<Mut<State>>,
}

impl Idle {
    fn new(state: &Ref<Mut<State>>) -> Self {
        let id = {
            let mut state = state.lock();
            if state.tasks.is_empty() {
                return Self(None);
            }
            state.idles.insert(None)
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
                let mut state = state.lock();
                state.idles.remove(data.id);
            }
        }
    }
}

impl Future for Idle {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(data) = &self.0 {
            if let Some(state) = data.state.upgrade() {
                let mut state = state.lock();
                if state.tasks.is_empty() {
                    state.idles.remove(data.id);
                } else {
                    state.idles[data.id] = Some(cx.waker().clone());
                    return Poll::Pending;
                }
            }
        }
        Poll::Ready(())
    }
}
