//! Task queue for spawned futures - optimized for both parallel and non-parallel modes

use alloc::{boxed::Box, collections::VecDeque};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

#[cfg(feature = "parallel")]
use std::sync::Mutex;

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

/// For parallel mode: uses std::sync::Mutex for the task queue
/// Futures don't need Send - they're only polled while holding the runtime lock
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
        self.inner.lock().unwrap().tasks.is_empty()
    }

    /// Push a task
    /// # Safety
    /// Caller must ensure future lifetime is valid
    pub unsafe fn push<F: Future<Output = ()>>(&self, future: F) {
        let future: BoxedTask =
            core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
        let mut inner = self.inner.lock().unwrap();
        inner.tasks.push_back(future);
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }

    pub fn listen(&self, waker: Waker) {
        self.inner.lock().unwrap().waker = Some(waker);
    }

    /// Poll tasks - caller must hold runtime lock for JS execution safety
    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        let mut inner = self.inner.lock().unwrap();

        if inner.tasks.is_empty() {
            return TaskPoll::Empty;
        }

        let mut made_progress = false;
        let count = inner.tasks.len();

        for _ in 0..count {
            let Some(mut task) = inner.tasks.pop_front() else {
                break;
            };

            // Drop lock while polling to avoid deadlock if task spawns more tasks
            drop(inner);
            let result = task.as_mut().poll(cx);
            inner = self.inner.lock().unwrap();

            match result {
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

// Safety: TaskQueue is only accessed while holding the runtime lock
// The Mutex is just for protecting the queue structure, not for cross-thread access
#[cfg(feature = "parallel")]
unsafe impl Send for TaskQueue {}
#[cfg(feature = "parallel")]
unsafe impl Sync for TaskQueue {}
