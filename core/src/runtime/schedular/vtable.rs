use std::{
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    ptr::NonNull,
    sync::Arc,
    task::{Context, Poll},
};

use super::Task;

#[derive(Debug, Clone)]
pub(crate) struct VTable {
    pub(crate) task_drive: unsafe fn(NonNull<Task<()>>, cx: &mut Context) -> Poll<()>,
    pub(crate) task_decr: unsafe fn(NonNull<Task<()>>),
    pub(crate) task_incr: unsafe fn(NonNull<Task<()>>),
    pub(crate) task_drop: unsafe fn(NonNull<Task<()>>),
}

impl VTable {
    pub const fn get<F: Future<Output = ()>>() -> &'static VTable {
        trait HasVTable {
            const V_TABLE: VTable;
        }

        impl<F: Future<Output = ()>> HasVTable for F {
            const V_TABLE: VTable = VTable {
                task_decr: VTable::decr::<F>,
                task_drive: VTable::drive::<F>,
                task_incr: VTable::incr::<F>,
                task_drop: VTable::drop::<F>,
            };
        }

        &<F as HasVTable>::V_TABLE
    }

    unsafe fn decr<F: Future<Output = ()>>(ptr: NonNull<Task<()>>) {
        Arc::decrement_strong_count(ptr.cast::<Task<F>>().as_ptr())
    }

    unsafe fn incr<F: Future<Output = ()>>(ptr: NonNull<Task<()>>) {
        Arc::increment_strong_count(ptr.cast::<Task<F>>().as_ptr())
    }

    unsafe fn drive<F: Future<Output = ()>>(ptr: NonNull<Task<()>>, cx: &mut Context) -> Poll<()> {
        let ptr = ptr.cast::<Task<F>>();
        Pin::new_unchecked(&mut *(*ptr.as_ref().future.get())).poll(cx)
    }

    unsafe fn drop<F: Future<Output = ()>>(ptr: NonNull<Task<()>>) {
        let ptr = ptr.cast::<Task<F>>();
        unsafe { ManuallyDrop::drop(&mut (*ptr.as_ref().future.get())) }
    }
}
