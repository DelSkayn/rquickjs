//! Task queue with per-task wakers and inline-small-future storage.
//!
//! # Locking model
//!
//! Everything touched *only* while the async runtime lock is held uses
//! [`UnsafeCell`]/`Cell` (tasks, free/chunk lists, slot vtable).
//! Fields that a `Waker` can touch from an arbitrary thread use
//! unconditional atomics (`Flag`, `len`) or mutexes (`Shared`).
//! This makes the task queue safe for cross-thread waker delivery
//! regardless of the `parallel` feature.
//!
//! # Task storage
//!
//! Each slot has an inline byte buffer (`INLINE_SIZE` bytes). Futures
//! that fit are written directly into the slot — no heap allocation.
//! Larger futures fall back to a `Pin<Box<dyn Future>>` stored in the
//! same space.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::{
    cell::{Cell, UnsafeCell},
    future::Future,
    marker::PhantomData,
    mem::{align_of, size_of, MaybeUninit},
    pin::Pin,
    ptr,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPoll {
    Empty,
    Pending,
    Progress,
}

const CHUNK: usize = 1024;
const INLINE_SIZE: usize = 64;
const INLINE_ALIGN: usize = 16;

#[derive(Default)]
struct Flag(AtomicBool);
impl Flag {
    #[inline]
    fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    #[inline]
    fn set(&self, v: bool) {
        self.0.store(v, Ordering::Release)
    }
    #[inline]
    fn try_set(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

struct Shared<T>(Mutex<T>);
impl<T> Shared<T> {
    #[inline]
    fn new(v: T) -> Self {
        Self(Mutex::new(v))
    }
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.lock())
    }
}

struct TaskVTable {
    poll: unsafe fn(*mut u8, &mut Context) -> Poll<()>,
    drop: unsafe fn(*mut u8),
}

struct InlineVT<F>(PhantomData<F>);
impl<F: Future<Output = ()>> InlineVT<F> {
    const V: TaskVTable = TaskVTable {
        poll: |p, cx| unsafe { Pin::new_unchecked(&mut *(p as *mut F)).poll(cx) },
        drop: |p| unsafe { ptr::drop_in_place(p as *mut F) },
    };
}

type HeapTask = Pin<Box<dyn Future<Output = ()>>>;
const HEAP_VTABLE: TaskVTable = TaskVTable {
    poll: |p, cx| unsafe { (*(p as *mut HeapTask)).as_mut().poll(cx) },
    drop: |p| unsafe { ptr::drop_in_place(p as *mut HeapTask) },
};

#[repr(align(16))]
struct Storage(#[allow(dead_code)] [MaybeUninit<u8>; INLINE_SIZE]);

struct Slot {
    vtable: Cell<Option<&'static TaskVTable>>,
    storage: UnsafeCell<Storage>,
    queued: Flag,
    active: Flag,
    queue: *const TaskQueue,
}

const _: () = assert!(size_of::<HeapTask>() <= INLINE_SIZE);
const _: () = assert!(align_of::<HeapTask>() <= INLINE_ALIGN);

pub struct TaskQueue {
    chunks: UnsafeCell<Vec<Box<[Slot; CHUNK]>>>,
    free: UnsafeCell<Vec<*mut Slot>>,
    len: AtomicU32,
    ready: Shared<Vec<*mut Slot>>,
    waker: Shared<Option<Waker>>,
    has_waker: Flag,
}

static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |p| RawWaker::new(p, &WAKER_VTABLE),
    |p| unsafe { wake(p) },
    |p| unsafe { wake(p) },
    |_| {},
);

unsafe fn wake(p: *const ()) {
    let slot = &*(p as *const Slot);
    if !slot.active.get() || !slot.queued.try_set() {
        return;
    }
    let queue = &*slot.queue;
    queue.ready.with(|r| r.push(p as *mut Slot));
    if !queue.has_waker.get() {
        return;
    }
    let waker = queue.waker.with(|w| w.clone());
    if let Some(w) = waker {
        w.wake_by_ref();
    }
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            chunks: UnsafeCell::new(Vec::new()),
            free: UnsafeCell::new(Vec::new()),
            len: AtomicU32::new(0),
            ready: Shared::new(Vec::new()),
            waker: Shared::new(None),
            has_waker: Flag::default(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::Acquire) == 0
    }

    fn alloc_slot(&self) -> *mut Slot {
        unsafe {
            let free = &mut *self.free.get();
            if let Some(p) = free.pop() {
                return p;
            }
            let chunks = &mut *self.chunks.get();
            let chunk: Box<[Slot; CHUNK]> = Box::new(core::array::from_fn(|_| Slot {
                vtable: Cell::new(None),
                storage: UnsafeCell::new(Storage([MaybeUninit::uninit(); INLINE_SIZE])),
                queued: Flag::default(),
                active: Flag::default(),
                queue: self,
            }));
            chunks.push(chunk);
            let chunk = chunks.last_mut().unwrap();
            let first = &mut chunk[0] as *mut Slot;
            free.reserve(CHUNK - 1);
            for s in chunk.iter_mut().skip(1) {
                free.push(s as *mut Slot);
            }
            first
        }
    }

    /// # Safety
    /// Any references captured by the future must remain valid until it
    /// completes or the queue is dropped.
    pub unsafe fn push<F: Future<Output = ()>>(&self, future: F) {
        let slot_ptr = self.alloc_slot();
        let slot = &*slot_ptr;
        let storage_ptr = slot.storage.get() as *mut u8;

        if size_of::<F>() <= INLINE_SIZE && align_of::<F>() <= INLINE_ALIGN {
            ptr::write(storage_ptr as *mut F, future);
            slot.vtable.set(Some(&InlineVT::<F>::V));
        } else {
            let boxed: HeapTask =
                core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
            ptr::write(storage_ptr as *mut HeapTask, boxed);
            slot.vtable.set(Some(&HEAP_VTABLE));
        }

        slot.active.set(true);
        slot.queued.set(true);
        self.ready.with(|r| r.push(slot_ptr));
        self.len.fetch_add(1, Ordering::Release);

        if self.has_waker.get() {
            let waker = self.waker.with(|w| w.take());
            self.has_waker.set(false);
            if let Some(w) = waker {
                w.wake();
            }
        }
    }

    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        self.waker.with(|w| {
            if !matches!(w, Some(old) if old.will_wake(cx.waker())) {
                *w = Some(cx.waker().clone());
            }
        });
        self.has_waker.set(true);

        if self.is_empty() {
            return TaskPoll::Empty;
        }

        let mut progress = false;
        while let Some(slot_ptr) = self.ready.with(|r| r.pop()) {
            let slot = unsafe { &*slot_ptr };
            slot.queued.set(false);

            let Some(vt) = slot.vtable.get() else {
                continue;
            };

            let storage_ptr = slot.storage.get() as *mut u8;
            let waker =
                unsafe { Waker::from_raw(RawWaker::new(slot_ptr as *const (), &WAKER_VTABLE)) };
            let done =
                unsafe { (vt.poll)(storage_ptr, &mut Context::from_waker(&waker)) }.is_ready();

            if done {
                unsafe { (vt.drop)(storage_ptr) };
                slot.vtable.set(None);
                slot.active.set(false);
                unsafe { (*self.free.get()).push(slot_ptr) };
                self.len.fetch_sub(1, Ordering::Release);
                progress = true;
            }
        }

        if self.is_empty() {
            TaskPoll::Empty
        } else if progress {
            TaskPoll::Progress
        } else {
            TaskPoll::Pending
        }
    }
}

impl Drop for TaskQueue {
    fn drop(&mut self) {
        let chunks = self.chunks.get_mut();
        for chunk in chunks.iter_mut() {
            for slot in chunk.iter() {
                slot.active.set(false);
                if let Some(vt) = slot.vtable.take() {
                    unsafe { (vt.drop)(slot.storage.get() as *mut u8) };
                }
            }
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for TaskQueue {}
unsafe impl Sync for TaskQueue {}
