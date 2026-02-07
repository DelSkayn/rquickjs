//! Task queue with per-task wakers using stable arena pointers

use alloc::{boxed::Box, vec::Vec};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(not(feature = "parallel"))]
use core::cell::{Cell, RefCell, UnsafeCell};

#[cfg(feature = "parallel")]
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(feature = "parallel")]
use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPoll {
    Empty,
    Pending,
    Progress,
}

type BoxedTask = Pin<Box<dyn Future<Output = ()>>>;
const CHUNK: usize = 1024;

#[cfg(not(feature = "parallel"))]
type QueuedFlag = Cell<bool>;
#[cfg(feature = "parallel")]
type QueuedFlag = AtomicBool;

#[cfg(not(feature = "parallel"))]
#[inline]
fn new_flag(val: bool) -> QueuedFlag {
    Cell::new(val)
}

#[cfg(feature = "parallel")]
#[inline]
fn new_flag(val: bool) -> QueuedFlag {
    AtomicBool::new(val)
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn try_set_true(flag: &QueuedFlag) -> bool {
    if !flag.get() {
        flag.set(true);
        true
    } else {
        false
    }
}

#[cfg(feature = "parallel")]
#[inline]
fn try_set_true(flag: &QueuedFlag) -> bool {
    flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
}

#[cfg(feature = "parallel")]
#[inline]
fn set_flag(flag: &QueuedFlag, val: bool) {
    flag.store(val, Ordering::Release)
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn set_flag(flag: &QueuedFlag, val: bool) {
    flag.set(val)
}

#[cfg(not(feature = "parallel"))]
struct Storage<T>(UnsafeCell<T>);
#[cfg(feature = "parallel")]
struct Storage<T>(Mutex<T>);

#[cfg(not(feature = "parallel"))]
struct WakerStorage(RefCell<Option<Waker>>);
#[cfg(feature = "parallel")]
struct WakerStorage(Mutex<Option<Waker>>);

#[cfg(not(feature = "parallel"))]
struct TaskStorage(RefCell<Option<BoxedTask>>);
#[cfg(feature = "parallel")]
struct TaskStorage(Mutex<Option<BoxedTask>>);

impl<T> Storage<T> {
    #[cfg(not(feature = "parallel"))]
    fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
    #[cfg(feature = "parallel")]
    fn new(val: T) -> Self {
        Self(Mutex::new(val))
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(unsafe { &mut *self.0.get() })
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.lock())
    }
}

impl WakerStorage {
    #[cfg(not(feature = "parallel"))]
    fn new(val: Option<Waker>) -> Self {
        Self(RefCell::new(val))
    }
    #[cfg(feature = "parallel")]
    fn new(val: Option<Waker>) -> Self {
        Self(Mutex::new(val))
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn take_clone(&self) -> Option<Waker> {
        self.0.borrow().clone()
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn take_clone(&self) -> Option<Waker> {
        self.0.lock().clone()
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn set(&self, val: Option<Waker>) {
        *self.0.borrow_mut() = val;
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn set(&self, val: Option<Waker>) {
        *self.0.lock() = val;
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn take(&self) -> Option<Waker> {
        self.0.borrow_mut().take()
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn take(&self) -> Option<Waker> {
        self.0.lock().take()
    }
}

impl TaskStorage {
    #[cfg(not(feature = "parallel"))]
    fn new(val: Option<BoxedTask>) -> Self {
        Self(RefCell::new(val))
    }
    #[cfg(feature = "parallel")]
    fn new(val: Option<BoxedTask>) -> Self {
        Self(Mutex::new(val))
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn set(&self, val: Option<BoxedTask>) {
        *self.0.borrow_mut() = val;
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn set(&self, val: Option<BoxedTask>) {
        *self.0.lock() = val;
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut Option<BoxedTask>) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut Option<BoxedTask>) -> R) -> R {
        f(&mut self.0.lock())
    }
}

#[cfg(not(feature = "parallel"))]
struct Counter(Cell<u32>);
#[cfg(feature = "parallel")]
struct Counter(AtomicU32);

impl Counter {
    #[cfg(not(feature = "parallel"))]
    fn new(val: u32) -> Self {
        Self(Cell::new(val))
    }
    #[cfg(feature = "parallel")]
    fn new(val: u32) -> Self {
        Self(AtomicU32::new(val))
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn get(&self) -> u32 {
        self.0.get()
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn get(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn inc(&self) {
        self.0.set(self.0.get().saturating_add(1))
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn inc(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    #[cfg(not(feature = "parallel"))]
    #[inline]
    fn dec(&self) {
        self.0.set(self.0.get().saturating_sub(1))
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn dec(&self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Slot {
    task: TaskStorage,
    queued: QueuedFlag,
    active: QueuedFlag, // true while task exists (even during poll)
    queue: *const TaskQueue,
}

pub struct TaskQueue {
    chunks: Storage<Vec<Box<[Slot; CHUNK]>>>,
    ready: Storage<Vec<*mut Slot>>,
    free: Storage<Vec<*mut Slot>>,
    len: Counter,
    waker: WakerStorage,
}

unsafe fn waker_clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &VTABLE)
}

unsafe fn waker_wake(p: *const ()) {
    waker_wake_by_ref(p);
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn get_flag(flag: &QueuedFlag) -> bool {
    flag.get()
}

#[cfg(feature = "parallel")]
#[inline]
fn get_flag(flag: &QueuedFlag) -> bool {
    flag.load(Ordering::Acquire)
}

unsafe fn waker_wake_by_ref(p: *const ()) {
    let slot = &*(p as *const Slot);
    if get_flag(&slot.active) && try_set_true(&slot.queued) {
        let queue = &*slot.queue;
        queue.ready.with(|r| r.push(p as *mut Slot));
        if let Some(w) = queue.waker.take_clone() {
            w.wake_by_ref();
        }
    }
}

unsafe fn waker_drop(_: *const ()) {}

static VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            chunks: Storage::new(Vec::new()),
            ready: Storage::new(Vec::new()),
            free: Storage::new(Vec::new()),
            len: Counter::new(0),
            waker: WakerStorage::new(None),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    fn alloc_slot(&self) -> *mut Slot {
        if let Some(ptr) = self.free.with(|f| f.pop()) {
            return ptr;
        }
        self.chunks.with(|chunks| {
            let chunk: Box<[Slot; CHUNK]> = Box::new(core::array::from_fn(|_| Slot {
                task: TaskStorage::new(None),
                queued: new_flag(false),
                active: new_flag(false),
                queue: self,
            }));
            chunks.push(chunk);
            let chunk = chunks.last_mut().unwrap();
            let first = &mut chunk[0] as *mut Slot;
            self.free.with(|free| {
                for i in 1..CHUNK {
                    free.push(&mut chunk[i]);
                }
            });
            first
        })
    }

    /// # Safety
    /// The future must be valid for the lifetime of the task queue or until it completes.
    pub unsafe fn push<F: Future<Output = ()>>(&self, future: F) {
        let future: BoxedTask =
            core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
        let slot_ptr = self.alloc_slot();
        let slot = &*slot_ptr;
        slot.task.set(Some(future));
        set_flag(&slot.active, true);
        set_flag(&slot.queued, true);
        self.ready.with(|r| r.push(slot_ptr));
        self.len.inc();
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }

    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        self.waker.set(Some(cx.waker().clone()));

        if self.is_empty() {
            return TaskPoll::Empty;
        }

        let mut progress = false;

        loop {
            let slot_ptr = match self.ready.with(|r| r.pop()) {
                Some(p) => p,
                None => break,
            };
            let slot = unsafe { &*slot_ptr };
            set_flag(&slot.queued, false);

            // Take task out to avoid holding lock during poll
            let mut task = match slot.task.with(|t| t.take()) {
                Some(t) => t,
                None => continue,
            };

            let w = unsafe { Waker::from_raw(RawWaker::new(slot_ptr as *const (), &VTABLE)) };
            let is_ready = task.as_mut().poll(&mut Context::from_waker(&w)) == Poll::Ready(());

            if is_ready {
                set_flag(&slot.active, false);
                self.free.with(|f| f.push(slot_ptr));
                self.len.dec();
                progress = true;
            } else {
                // Put task back
                slot.task.set(Some(task));
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
        self.chunks.with(|chunks| {
            for chunk in chunks.iter_mut() {
                for slot in chunk.iter() {
                    slot.task.set(None);
                    set_flag(&slot.active, false);
                }
            }
        });
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
