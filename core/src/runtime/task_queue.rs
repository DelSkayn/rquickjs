//! Task queue for spawned futures - optimized for both parallel and non-parallel modes

#[cfg(not(feature = "parallel"))]
use alloc::{boxed::Box, collections::VecDeque};
#[cfg(feature = "parallel")]
use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

#[cfg(feature = "parallel")]
use parking_lot::Mutex;

#[cfg(not(feature = "parallel"))]
use core::cell::UnsafeCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPoll {
    Empty,
    Pending,
    Progress,
    Done,
}

type BoxedTask = Pin<Box<dyn Future<Output = ()>>>;

#[cfg(feature = "parallel")]
pub struct TaskQueue {
    inner: Mutex<TaskQueueInner>,
}

#[cfg(feature = "parallel")]
struct TaskQueueInner {
    tasks: VecDeque<BoxedTask>,
    waker: Option<Waker>,
}

#[cfg(not(feature = "parallel"))]
pub struct TaskQueue {
    inner: UnsafeCell<TaskQueueInner>,
}

#[cfg(not(feature = "parallel"))]
struct TaskQueueInner {
    tasks: VecDeque<BoxedTask>,
    waker: Option<Waker>,
}

#[cfg(feature = "parallel")]
impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            inner: Mutex::new(TaskQueueInner {
                tasks: VecDeque::new(),
                waker: None,
            }),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().tasks.is_empty()
    }

    /// # Safety
    /// Caller must ensure future lifetime is valid
    pub unsafe fn push<F: Future<Output = ()>>(&self, future: F) {
        let future: BoxedTask =
            core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
        let mut inner = self.inner.lock();
        inner.tasks.push_back(future);
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }

    pub fn listen(&self, waker: Waker) {
        self.inner.lock().waker = Some(waker);
    }

    /// Poll tasks - optimized to minimize lock contention
    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        // Take all tasks out in one lock acquisition
        let mut batch: Vec<BoxedTask> = {
            let mut inner = self.inner.lock();
            if inner.tasks.is_empty() {
                return TaskPoll::Empty;
            }
            inner.tasks.drain(..).collect()
        };

        let mut made_progress = false;
        let mut pending = Vec::new();

        // Poll all tasks without holding the lock
        for mut task in batch.drain(..) {
            match task.as_mut().poll(cx) {
                Poll::Ready(()) => made_progress = true,
                Poll::Pending => pending.push(task),
            }
        }

        // Put pending tasks back in one lock acquisition
        if !pending.is_empty() {
            let mut inner = self.inner.lock();
            for task in pending.into_iter().rev() {
                inner.tasks.push_front(task);
            }
        }

        // Check if new tasks were spawned during polling
        let has_tasks = !self.inner.lock().tasks.is_empty();

        if !has_tasks {
            if made_progress {
                TaskPoll::Done
            } else {
                TaskPoll::Empty
            }
        } else if made_progress {
            TaskPoll::Progress
        } else {
            TaskPoll::Pending
        }
    }
}

#[cfg(not(feature = "parallel"))]
impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            inner: UnsafeCell::new(TaskQueueInner {
                tasks: VecDeque::new(),
                waker: None,
            }),
        }
    }

    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut TaskQueueInner {
        unsafe { &mut *self.inner.get() }
    }

    pub fn is_empty(&self) -> bool {
        self.inner().tasks.is_empty()
    }

    /// # Safety
    /// Caller must ensure future lifetime is valid
    pub unsafe fn push<F: Future<Output = ()>>(&self, future: F) {
        let future: BoxedTask =
            core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
        let inner = self.inner();
        inner.tasks.push_back(future);
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }

    pub fn listen(&self, waker: Waker) {
        self.inner().waker = Some(waker);
    }

    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        let inner = self.inner();

        if inner.tasks.is_empty() {
            return TaskPoll::Empty;
        }

        let mut made_progress = false;
        let count = inner.tasks.len();

        for _ in 0..count {
            let Some(mut task) = inner.tasks.pop_front() else {
                break;
            };

            match task.as_mut().poll(cx) {
                Poll::Ready(()) => made_progress = true,
                Poll::Pending => inner.tasks.push_back(task),
            }
        }

        if inner.tasks.is_empty() {
            if made_progress {
                TaskPoll::Done
            } else {
                TaskPoll::Empty
            }
        } else if made_progress {
            TaskPoll::Progress
        } else {
            TaskPoll::Pending
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "parallel")]
unsafe impl Send for TaskQueue {}
#[cfg(feature = "parallel")]
unsafe impl Sync for TaskQueue {}
