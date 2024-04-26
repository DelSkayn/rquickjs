use std::{
    cell::{Cell, UnsafeCell},
    future::Future,
    mem::{offset_of, ManuallyDrop},
    pin::Pin,
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{Context, Poll},
};

mod queue;
use queue::Queue;

mod vtable;
use vtable::VTable;

mod waker;

mod atomic_waker;

use self::queue::NodeHeader;

use std::ops::{Deref, DerefMut};

pub struct Defer<T, F: FnOnce(&mut T)> {
    value: ManuallyDrop<T>,
    f: Option<F>,
}

impl<T, F: FnOnce(&mut T)> Defer<T, F> {
    pub fn new(value: T, func: F) -> Self {
        Defer {
            value: ManuallyDrop::new(value),
            f: Some(func),
        }
    }

    pub fn take(mut self) -> T {
        self.f = None;
        unsafe { ManuallyDrop::take(&mut self.value) }
    }
}

impl<T, F: FnOnce(&mut T)> Deref for Defer<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, F: FnOnce(&mut T)> DerefMut for Defer<T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T, F> Drop for Defer<T, F>
where
    F: FnOnce(&mut T),
{
    fn drop(&mut self) {
        if let Some(x) = self.f.take() {
            (x)(&mut *self.value);
            unsafe { ManuallyDrop::drop(&mut self.value) }
        }
    }
}

#[repr(C)]
struct Task<F> {
    head: NodeHeader,
    body: TaskBody,
    future: UnsafeCell<F>,
}

// Seperate struct to not have everything be repr(C)
struct TaskBody {
    queue: Weak<Queue>,
    vtable: &'static VTable,
    // The double linked list of tasks.
    next: Cell<Option<NonNull<Task<u8>>>>,
    prev: Cell<Option<NonNull<Task<u8>>>>,
    // wether the task is currently in the queue to be re-polled.
    queued: AtomicBool,
    done: Cell<bool>,
}

pub struct Schedular {
    len: Cell<usize>,
    should_poll: Arc<Queue>,
    all_next: Cell<Option<NonNull<Task<u8>>>>,
    all_prev: Cell<Option<NonNull<Task<u8>>>>,
}

impl Schedular {
    pub fn new() -> Self {
        let queue = Arc::new(Queue::new());
        unsafe {
            Pin::new_unchecked(&*queue).init();
        }
        Schedular {
            len: Cell::new(0),
            should_poll: queue,
            all_prev: Cell::new(None),
            all_next: Cell::new(None),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.all_next.get().is_none()
    }

    /// # Safety
    /// This function erases any lifetime associated with the future.
    /// Caller must ensure that either the future completes or is dropped before the lifetime
    pub unsafe fn push<F>(&self, f: F)
    where
        F: Future<Output = ()>,
    {
        let queue = Arc::downgrade(&self.should_poll);

        debug_assert_eq!(offset_of!(Task<F>, future), offset_of!(Task<u8>, future));

        let task = Arc::new(Task {
            head: NodeHeader::new(),
            body: TaskBody {
                queue,
                vtable: VTable::get::<F>(),
                next: Cell::new(None),
                prev: Cell::new(None),
                queued: AtomicBool::new(true),
                done: Cell::new(false),
            },
            future: UnsafeCell::new(ManuallyDrop::new(f)),
        });

        // One count for the all list and one for the should_poll list.
        let task = NonNull::new_unchecked(Arc::into_raw(task) as *mut Task<F>).cast::<Task<u8>>();
        Arc::increment_strong_count(task.as_ptr());

        self.push_task_to_all(task);

        Pin::new_unchecked(&*self.should_poll).push(task.cast());
        self.len.set(self.len.get() + 1);
    }

    unsafe fn push_task_to_all(&self, task: NonNull<Task<u8>>) {
        task.as_ref().body.next.set(self.all_next.get());

        if let Some(x) = self.all_next.get() {
            x.as_ref().body.prev.set(Some(task));
        }
        self.all_next.set(Some(task));
        if self.all_prev.get().is_none() {
            self.all_prev.set(Some(task));
        }
    }

    unsafe fn pop_task_all(&self, task: NonNull<Task<u8>>) {
        task.as_ref().body.queued.store(true, Ordering::Release);
        task.as_ref().body.done.set(true);

        // detach the task from the all list
        if let Some(next) = task.as_ref().body.next.get() {
            next.as_ref().body.prev.set(task.as_ref().body.prev.get())
        } else {
            self.all_prev.set(task.as_ref().body.prev.get());
        }
        if let Some(prev) = task.as_ref().body.prev.get() {
            prev.as_ref().body.next.set(task.as_ref().body.next.get())
        } else {
            self.all_next.set(task.as_ref().body.next.get());
        }

        // drop the ownership of the all list,
        // Task is now dropped or only owned by wakers or
        Self::drop_task(task);
        self.len.set(self.len.get() - 1);
    }

    unsafe fn drop_task(ptr: NonNull<Task<u8>>) {
        (ptr.as_ref().body.vtable.task_drop)(ptr)
    }

    unsafe fn drive_task(ptr: NonNull<Task<u8>>, ctx: &mut Context) -> Poll<()> {
        (ptr.as_ref().body.vtable.task_drive)(ptr, ctx)
    }

    pub unsafe fn poll(&self, cx: &mut Context) -> Poll<bool> {
        // During polling ownership is upheld by making sure arc counts are properly tranfered.
        // Both ques, should_poll and all, have ownership of the arc count.
        // Whenever a task is pushed onto the should_poll queue ownership is transfered.
        // During task pusing into the schedular ownership of the count was transfered into the all
        // list.

        if self.is_empty() {
            // No tasks, nothing to be done.
            return Poll::Ready(false);
        }

        self.should_poll.waker().register(cx.waker());

        let mut iteration = 0;
        let mut yielded = 0;
        let mut pending = false;

        loop {
            // Popped a task, ownership taken from the que
            let cur = match Pin::new_unchecked(&*self.should_poll).pop() {
                queue::Pop::Empty => {
                    if pending {
                        return Poll::Pending;
                    } else {
                        return Poll::Ready(iteration > 0);
                    }
                }
                queue::Pop::Value(x) => x,
                queue::Pop::Inconsistant => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            };

            let cur = cur.cast::<Task<u8>>();

            if cur.as_ref().body.done.get() {
                // Task was already done, we con drop the ownership we got from the que.
                Self::drop_task(cur);
                continue;
            }

            let prev = cur.as_ref().body.queued.swap(false, Ordering::AcqRel);
            assert!(prev);

            // ownership transfered into the waker, which won't drop until the iteration completes.
            let waker = waker::get(cur);
            // if drive_task panics we still want to remove the task from the list.
            // So handle it with a drop
            let remove = Defer::new(self, |this| (*this).pop_task_all(cur));
            let mut ctx = Context::from_waker(&waker);

            iteration += 1;

            match Self::drive_task(cur, &mut ctx) {
                Poll::Ready(_) => {
                    // Nothing todo the defer will remove the task from the list.
                }
                Poll::Pending => {
                    // don't remove task from the list.
                    remove.take();
                    pending = true;
                    yielded += cur.as_ref().body.queued.load(Ordering::Relaxed) as usize;
                    if yielded > 2 || iteration > self.len.get() {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                }
            }
        }
    }

    pub fn clear(&self) {
        // Clear all pending futures from the all list
        let mut cur = self.all_next.get();
        while let Some(c) = cur {
            unsafe {
                cur = c.as_ref().body.next.get();
                self.pop_task_all(c)
            }
        }

        loop {
            let cur = match unsafe { Pin::new_unchecked(&*self.should_poll).pop() } {
                queue::Pop::Empty => break,
                queue::Pop::Value(x) => x,
                queue::Pop::Inconsistant => {
                    std::thread::yield_now();
                    continue;
                }
            };

            unsafe { Self::drop_task(cur.cast()) };
        }
    }
}

impl Drop for Schedular {
    fn drop(&mut self) {
        self.clear()
    }
}
