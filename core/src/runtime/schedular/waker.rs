use std::{
    pin::Pin,
    ptr::NonNull,
    sync::{atomic::Ordering, Arc},
    task::{RawWaker, RawWakerVTable, Waker},
};

use super::Task;

unsafe fn schedular_clone(ptr: *const ()) -> RawWaker {
    Arc::increment_strong_count(ptr.cast::<Task<u8>>());
    RawWaker::new(ptr.cast(), &SCHEDULAR_WAKER_V_TABLE)
}

unsafe fn schedular_wake(ptr: *const ()) {
    let task = NonNull::new_unchecked(ptr as *mut ()).cast::<Task<u8>>();

    if task
        .as_ref()
        .body
        .queued
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        // Already awoken, skip!
        schedular_drop(ptr);
        return;
    }

    // retrieve the queue, if already dropped, just return as we don't need to awake anything.
    let Some(queue) = task.as_ref().body.queue.upgrade() else {
        schedular_drop(ptr);
        return;
    };

    // push to the que
    Pin::new_unchecked(&*queue).push(task.cast());

    // wake up the schedular.
    queue.waker().wake()
}

unsafe fn schedular_wake_ref(ptr: *const ()) {
    Arc::increment_strong_count(ptr.cast::<Task<u8>>());
    schedular_wake(ptr)
}

unsafe fn schedular_drop(ptr: *const ()) {
    let ptr = ptr.cast::<Task<u8>>();
    ((*ptr).body.vtable.task_drop)(NonNull::new_unchecked(ptr as *mut _))
}

static SCHEDULAR_WAKER_V_TABLE: RawWakerVTable = RawWakerVTable::new(
    schedular_clone,
    schedular_wake,
    schedular_wake_ref,
    schedular_drop,
);

pub unsafe fn get(ptr: NonNull<Task<u8>>) -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(ptr.as_ptr().cast(), &SCHEDULAR_WAKER_V_TABLE)) }
}
