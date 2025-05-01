use alloc::sync::{Arc, Weak};
use core::{
    cell::{Cell, UnsafeCell},
    future::Future,
    mem::ManuallyDrop,
    ptr::{addr_of_mut, NonNull},
    sync::atomic::AtomicBool,
    task::{Context, Poll},
};

use super::{
    queue::{NodeHeader, Queue},
    vtable::VTable,
};

#[repr(C)]
pub struct Task<F> {
    /// Header for the intrusive list,
    /// Must be first.
    pub(crate) head: NodeHeader,
    /// Data the schedular uses to run the future.
    pub(crate) body: TaskBody,
    /// The future itself.
    pub(crate) future: UnsafeCell<ManuallyDrop<F>>,
}

impl<F: Future<Output = ()>> Task<F> {
    pub fn new(queue: Weak<Queue>, f: F) -> Self {
        Self {
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
        }
    }
}

// Seperate struct to not have everything in task be repr(C)
pub struct TaskBody {
    pub(crate) queue: Weak<Queue>,
    pub(crate) vtable: &'static VTable,
    // The double linked list of tasks.
    pub(crate) next: Cell<Option<ErasedTaskPtr>>,
    pub(crate) prev: Cell<Option<ErasedTaskPtr>>,
    // wether the task is currently in the queue to be re-polled.
    pub(crate) queued: AtomicBool,
    pub(crate) done: Cell<bool>,
}

/// A raw pointer to a task with it's type erased.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ErasedTaskPtr(NonNull<Task<()>>);

impl ErasedTaskPtr {
    pub fn from_nonnull(ptr: NonNull<Task<()>>) -> Self {
        Self(ptr)
    }

    pub unsafe fn body<'a>(self) -> &'a TaskBody {
        let ptr = self.0.as_ptr();
        unsafe { &*addr_of_mut!((*ptr).body) }
    }

    pub fn as_node_ptr(self) -> NonNull<NodeHeader> {
        self.0.cast()
    }

    pub fn as_nonnull(self) -> NonNull<Task<()>> {
        self.0
    }

    pub unsafe fn task_drive(self, cx: &mut Context) -> Poll<()> {
        unsafe { (self.body().vtable.task_drive)(self.0, cx) }
    }

    pub unsafe fn task_incr(self) {
        unsafe { (self.body().vtable.task_incr)(self.0) }
    }

    pub unsafe fn task_decr(self) {
        unsafe { (self.body().vtable.task_decr)(self.0) }
    }

    pub unsafe fn task_drop(self) {
        unsafe { (self.body().vtable.task_drop)(self.0) }
    }
}

/// An owning pointer to a task with it's type erased.
pub struct ErasedTask(ErasedTaskPtr);

impl ErasedTask {
    pub unsafe fn from_ptr(ptr: ErasedTaskPtr) -> Self {
        Self(ptr)
    }

    pub fn into_ptr(this: Self) -> ErasedTaskPtr {
        let res = this.0;
        core::mem::forget(this);
        res
    }

    pub fn new<F>(task: Arc<Task<F>>) -> Self {
        unsafe {
            let ptr = NonNull::new_unchecked(Arc::into_raw(task) as *mut Task<F>).cast();
            Self(ErasedTaskPtr(ptr))
        }
    }

    pub fn body(&self) -> &TaskBody {
        unsafe { self.0.body() }
    }
}

impl Clone for ErasedTask {
    fn clone(&self) -> Self {
        unsafe {
            self.0.task_incr();
            Self(self.0)
        }
    }
}

impl Drop for ErasedTask {
    fn drop(&mut self) {
        unsafe { self.0.task_decr() }
    }
}
