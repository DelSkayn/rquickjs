use std::{
    future::Future,
    pin::Pin,
    ptr::NonNull,
    sync::Arc,
    task::{Context, Poll},
};

use super::Task;

#[derive(Debug, Clone)]
pub(crate) struct VTable {
    pub(crate) task_drive: unsafe fn(NonNull<Task<u8>>, cx: &mut Context) -> Poll<()>,
    pub(crate) task_drop: unsafe fn(NonNull<Task<u8>>),
}

impl VTable {
    pub const fn get<F: Future<Output = ()>>() -> &'static VTable {
        trait HasVTable {
            const V_TABLE: VTable;
        }

        impl<F: Future<Output = ()>> HasVTable for F {
            const V_TABLE: VTable = VTable {
                task_drop: VTable::drop::<F>,
                task_drive: VTable::drive::<F>,
            };
        }

        &<F as HasVTable>::V_TABLE
    }

    unsafe fn drop<F: Future<Output = ()>>(ptr: NonNull<Task<u8>>) {
        Arc::decrement_strong_count(ptr.cast::<Task<F>>().as_ptr())
    }

    unsafe fn drive<F: Future<Output = ()>>(ptr: NonNull<Task<u8>>, cx: &mut Context) -> Poll<()> {
        let ptr = ptr.cast::<Task<F>>();
        Pin::new_unchecked(&mut (*ptr.as_ref().future.get())).poll(cx)
    }
}
