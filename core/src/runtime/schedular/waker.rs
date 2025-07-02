use core::{
    pin::Pin,
    ptr::NonNull,
    sync::atomic::Ordering,
    task::{RawWaker, RawWakerVTable, Waker},
};

use super::{
    task::{ErasedTask, ErasedTaskPtr},
    Task,
};

unsafe fn inner_clone(ptr: *const ()) {
    let task_ptr = ptr.cast::<Task<()>>();
    ErasedTaskPtr::from_nonnull(NonNull::new_unchecked(task_ptr as *mut Task<()>)).task_incr();
}

unsafe fn schedular_clone(ptr: *const ()) -> RawWaker {
    inner_clone(ptr);
    RawWaker::new(ptr, &SCHEDULAR_WAKER_V_TABLE)
}

unsafe fn schedular_wake(ptr: *const ()) {
    // We have ownership so take it.
    let task = NonNull::new_unchecked(ptr as *mut ()).cast::<Task<()>>();
    let task = ErasedTaskPtr::from_nonnull(task);
    let task = ErasedTask::from_ptr(task);

    if task
        .body()
        .queued
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    // retrieve the queue, if already dropped, just return as we don't need to awake anything.
    let Some(queue) = task.body().queue.upgrade() else {
        return;
    };

    // push to the que
    Pin::new_unchecked(&*queue).push(ErasedTask::into_ptr(task).as_node_ptr());

    // wake up the schedular.
    queue.waker().wake()
}

unsafe fn schedular_wake_ref(ptr: *const ()) {
    inner_clone(ptr);
    schedular_wake(ptr)
}

unsafe fn schedular_drop(ptr: *const ()) {
    let task_ptr = (ptr as *mut ()).cast();
    ErasedTaskPtr::from_nonnull(NonNull::new_unchecked(task_ptr)).task_decr();
}

static SCHEDULAR_WAKER_V_TABLE: RawWakerVTable = RawWakerVTable::new(
    schedular_clone,
    schedular_wake,
    schedular_wake_ref,
    schedular_drop,
);

pub unsafe fn get(ptr: ErasedTask) -> Waker {
    let ptr = ErasedTask::into_ptr(ptr).as_nonnull().as_ptr();
    unsafe { Waker::from_raw(RawWaker::new(ptr.cast(), &SCHEDULAR_WAKER_V_TABLE)) }
}
