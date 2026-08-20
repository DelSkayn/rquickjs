//! Arena-backed task queue with inline-small-future storage.
//!
//! Ready futures avoid per-task allocation: small futures live directly in an
//! arena slot and the polling waker borrows that stable slot. A future that
//! clones its waker lazily gets one shared wake handle. The handle keeps the
//! arena alive and carries a slot generation, so stale wakers cannot target a
//! reused slot.

use alloc::{boxed::Box, sync::Arc, vec::Vec};
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

const CHUNK_SIZE: usize = 64;
const INLINE_SIZE: usize = 64;
const INLINE_ALIGN: usize = 16;

type HeapTask = Pin<Box<dyn Future<Output = ()>>>;

#[derive(Default)]
struct Flag(AtomicBool);

impl Flag {
    #[inline]
    fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    #[inline]
    fn set(&self, value: bool) {
        self.0.store(value, Ordering::Release)
    }

    #[inline]
    fn try_set(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

struct TaskVTable {
    poll: unsafe fn(*mut u8, &mut Context) -> Poll<()>,
    drop: unsafe fn(*mut u8),
}

struct InlineTask<F>(PhantomData<F>);

impl<F: Future<Output = ()>> InlineTask<F> {
    const VTABLE: TaskVTable = TaskVTable {
        poll: |ptr, cx| unsafe { Pin::new_unchecked(&mut *(ptr as *mut F)).poll(cx) },
        drop: |ptr| unsafe { ptr::drop_in_place(ptr as *mut F) },
    };
}

const HEAP_VTABLE: TaskVTable = TaskVTable {
    poll: |ptr, cx| unsafe { (*(ptr as *mut HeapTask)).as_mut().poll(cx) },
    drop: |ptr| unsafe { ptr::drop_in_place(ptr as *mut HeapTask) },
};

#[repr(align(16))]
struct TaskStorage(#[allow(dead_code)] [MaybeUninit<u8>; INLINE_SIZE]);

#[derive(Clone, Copy)]
struct TaskKey {
    slot: *mut Slot,
    generation: u32,
}

// Slots remain allocated while Inner is alive and are accessed across threads
// only through atomic fields and Inner::schedule.
unsafe impl Send for TaskKey {}
unsafe impl Sync for TaskKey {}

struct QueueState {
    ready: Vec<TaskKey>,
    waker: Option<Waker>,
}

struct Inner {
    alive: AtomicBool,
    has_waker: AtomicBool,
    state: Mutex<QueueState>,
    chunks: UnsafeCell<Vec<Box<[Slot; CHUNK_SIZE]>>>,
    free: UnsafeCell<Vec<*mut Slot>>,
    spawned: UnsafeCell<Vec<TaskKey>>,
    len: Cell<u32>,
}

// Non-atomic fields are accessed only while the async runtime lock is held.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Inner {
    fn new() -> Self {
        Self {
            alive: AtomicBool::new(true),
            has_waker: AtomicBool::new(false),
            state: Mutex::new(QueueState {
                ready: Vec::new(),
                waker: None,
            }),
            chunks: UnsafeCell::new(Vec::new()),
            free: UnsafeCell::new(Vec::new()),
            spawned: UnsafeCell::new(Vec::new()),
            len: Cell::new(0),
        }
    }

    fn take_waker(&self) -> Option<Waker> {
        if self.has_waker.swap(false, Ordering::AcqRel) {
            self.state.lock().waker.take()
        } else {
            None
        }
    }

    unsafe fn schedule(&self, key: TaskKey) {
        let slot = &*key.slot;
        if !self.alive.load(Ordering::Acquire)
            || slot.generation.load(Ordering::Acquire) != key.generation
            || !slot.active.get()
            || !slot.queued.try_set()
        {
            return;
        }

        let waker = {
            let mut state = self.state.lock();
            if !self.alive.load(Ordering::Acquire)
                || slot.generation.load(Ordering::Acquire) != key.generation
                || !slot.active.get()
            {
                // Do not clear the flag if the slot has already been reused.
                if slot.generation.load(Ordering::Acquire) == key.generation {
                    slot.queued.set(false);
                }
                return;
            }
            state.ready.push(key);
            let waker = state.waker.take();
            self.has_waker.store(false, Ordering::Release);
            waker
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

struct WakeHandle {
    inner: Arc<Inner>,
    key: TaskKey,
}

unsafe impl Send for WakeHandle {}
unsafe impl Sync for WakeHandle {}

struct Slot {
    vtable: Cell<Option<&'static TaskVTable>>,
    storage: UnsafeCell<TaskStorage>,
    wake_handle: UnsafeCell<Option<Arc<WakeHandle>>>,
    queued: Flag,
    active: Flag,
    generation: AtomicU32,
    inner: *const Inner,
}

impl Slot {
    fn new(inner: *const Inner) -> Self {
        Self {
            vtable: Cell::new(None),
            storage: UnsafeCell::new(TaskStorage([MaybeUninit::uninit(); INLINE_SIZE])),
            wake_handle: UnsafeCell::new(None),
            queued: Flag::default(),
            active: Flag::default(),
            generation: AtomicU32::new(0),
            inner,
        }
    }

    fn key(&self) -> TaskKey {
        TaskKey {
            slot: ptr::from_ref(self).cast_mut(),
            generation: self.generation.load(Ordering::Acquire),
        }
    }
}

const _: () = assert!(size_of::<HeapTask>() <= INLINE_SIZE);
const _: () = assert!(align_of::<HeapTask>() <= INLINE_ALIGN);

static BORROWED_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| unsafe { clone_borrowed(ptr) },
    |ptr| unsafe { wake_borrowed(ptr) },
    |ptr| unsafe { wake_borrowed(ptr) },
    |_| {},
);

static OWNED_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| unsafe { clone_owned(ptr) },
    |ptr| unsafe { wake_owned(ptr) },
    |ptr| unsafe { wake_owned_by_ref(ptr) },
    |ptr| unsafe { drop_owned(ptr) },
);
unsafe fn clone_borrowed(ptr: *const ()) -> RawWaker {
    let slot = &*(ptr as *const Slot);
    let cached = &mut *slot.wake_handle.get();
    let handle = cached.get_or_insert_with(|| {
        Arc::increment_strong_count(slot.inner);
        Arc::new(WakeHandle {
            inner: Arc::from_raw(slot.inner),
            key: slot.key(),
        })
    });
    RawWaker::new(Arc::into_raw(handle.clone()).cast(), &OWNED_WAKER_VTABLE)
}

unsafe fn wake_borrowed(ptr: *const ()) {
    let slot = &*(ptr as *const Slot);
    (*slot.inner).schedule(slot.key());
}

unsafe fn clone_owned(ptr: *const ()) -> RawWaker {
    Arc::increment_strong_count(ptr as *const WakeHandle);
    RawWaker::new(ptr, &OWNED_WAKER_VTABLE)
}

unsafe fn wake_owned(ptr: *const ()) {
    let handle = Arc::from_raw(ptr as *const WakeHandle);
    handle.inner.schedule(handle.key);
}

unsafe fn wake_owned_by_ref(ptr: *const ()) {
    let handle = &*(ptr as *const WakeHandle);
    handle.inner.schedule(handle.key);
}

unsafe fn drop_owned(ptr: *const ()) {
    Arc::decrement_strong_count(ptr as *const WakeHandle);
}
pub struct TaskQueue {
    inner: Arc<Inner>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.len.get() == 0
    }

    fn alloc_slot(&self) -> *mut Slot {
        unsafe {
            let free = &mut *self.inner.free.get();
            while let Some(slot) = free.pop() {
                // Retire exhausted slots so a stale generation can never wrap
                // around and match a future task.
                if (*slot).generation.load(Ordering::Relaxed) != u32::MAX {
                    return slot;
                }
            }

            let inner = Arc::as_ptr(&self.inner);
            let chunks = &mut *self.inner.chunks.get();
            chunks.push(Box::new(core::array::from_fn(|_| Slot::new(inner))));
            let chunk = chunks.last_mut().unwrap();
            let first = &mut chunk[0] as *mut Slot;
            free.reserve(CHUNK_SIZE - 1);
            for slot in chunk.iter_mut().skip(1) {
                free.push(slot);
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
        debug_assert!(slot.vtable.get().is_none());
        debug_assert!((*slot.wake_handle.get()).is_none());
        debug_assert!(!slot.active.get());

        let generation = slot
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        let key = TaskKey {
            slot: slot_ptr,
            generation,
        };
        let storage_ptr = slot.storage.get().cast::<u8>();
        if size_of::<F>() <= INLINE_SIZE && align_of::<F>() <= INLINE_ALIGN {
            ptr::write(storage_ptr.cast::<F>(), future);
            slot.vtable.set(Some(&InlineTask::<F>::VTABLE));
        } else {
            let boxed: HeapTask =
                core::mem::transmute(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + '_>>);
            ptr::write(storage_ptr.cast::<HeapTask>(), boxed);
            slot.vtable.set(Some(&HEAP_VTABLE));
        }

        slot.active.set(true);
        slot.queued.set(true);
        self.inner.len.set(
            self.inner
                .len
                .get()
                .checked_add(1)
                .expect("too many spawned tasks"),
        );
        (*self.inner.spawned.get()).push(key);

        if let Some(waker) = self.inner.take_waker() {
            waker.wake();
        }
    }

    pub fn poll(&self, cx: &mut Context) -> TaskPoll {
        // Reawakened tasks form a fixed batch, so a self-waking task is
        // polled at most once. Newly spawned tasks remain depth-first to keep
        // arena reuse and recursive-spawn throughput.
        let mut ready = {
            let mut state = self.inner.state.lock();
            if !matches!(&state.waker, Some(old) if old.will_wake(cx.waker())) {
                state.waker = Some(cx.waker().clone());
            }
            self.inner.has_waker.store(true, Ordering::Release);
            core::mem::take(&mut state.ready)
        };

        if self.is_empty() {
            return TaskPoll::Empty;
        }

        let mut progress = false;
        loop {
            let key = unsafe { (*self.inner.spawned.get()).pop() }.or_else(|| ready.pop());
            let Some(key) = key else { break };
            let slot = unsafe { &*key.slot };
            if slot.generation.load(Ordering::Acquire) != key.generation || !slot.active.get() {
                continue;
            }
            slot.queued.set(false);

            let Some(vtable) = slot.vtable.get() else {
                continue;
            };
            let storage_ptr = slot.storage.get().cast::<u8>();
            let raw = RawWaker::new(key.slot.cast(), &BORROWED_WAKER_VTABLE);
            let waker = unsafe { Waker::from_raw(raw) };
            let done =
                unsafe { (vtable.poll)(storage_ptr, &mut Context::from_waker(&waker)).is_ready() };
            drop(waker);

            if done {
                unsafe { (vtable.drop)(storage_ptr) };
                slot.vtable.set(None);
                slot.active.set(false);
                slot.queued.set(false);
                unsafe { (*slot.wake_handle.get()).take() };
                self.inner.len.set(self.inner.len.get() - 1);
                unsafe { (*self.inner.free.get()).push(key.slot) };
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
        self.inner.alive.store(false, Ordering::Release);
        self.inner.has_waker.store(false, Ordering::Release);
        unsafe { (*self.inner.spawned.get()).clear() };
        let mut state = self.inner.state.lock();
        state.ready.clear();
        state.waker = None;
        drop(state);

        unsafe {
            for chunk in &*self.inner.chunks.get() {
                for slot in chunk.iter() {
                    slot.active.set(false);
                    slot.queued.set(false);
                    (*slot.wake_handle.get()).take();
                    if let Some(vtable) = slot.vtable.take() {
                        (vtable.drop)(slot.storage.get().cast());
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    fn poll(queue: &TaskQueue) -> TaskPoll {
        let waker = Waker::noop();
        queue.poll(&mut Context::from_waker(waker))
    }

    struct CaptureWaker {
        captured: Arc<StdMutex<Option<Waker>>>,
        ready: bool,
    }

    impl Future for CaptureWaker {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            *self.captured.lock().unwrap() = Some(cx.waker().clone());
            if self.ready {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    struct CountPending(Arc<AtomicUsize>);

    impl Future for CountPending {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Poll::Pending
        }
    }

    #[test]
    fn stale_waker_does_not_wake_reused_slot() {
        let queue = TaskQueue::new();
        let captured = Arc::new(StdMutex::new(None));
        unsafe {
            queue.push(CaptureWaker {
                captured: captured.clone(),
                ready: true,
            });
        }
        assert_eq!(poll(&queue), TaskPoll::Empty);

        let polls = Arc::new(AtomicUsize::new(0));
        unsafe { queue.push(CountPending(polls.clone())) };
        assert_eq!(poll(&queue), TaskPoll::Pending);
        assert_eq!(polls.load(Ordering::Relaxed), 1);

        captured.lock().unwrap().as_ref().unwrap().wake_by_ref();
        assert_eq!(poll(&queue), TaskPoll::Pending);
        assert_eq!(polls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn waker_may_outlive_queue() {
        let captured = Arc::new(StdMutex::new(None));
        {
            let queue = TaskQueue::new();
            unsafe {
                queue.push(CaptureWaker {
                    captured: captured.clone(),
                    ready: true,
                });
            }
            assert_eq!(poll(&queue), TaskPoll::Empty);
        }

        let stale = captured.lock().unwrap().take().unwrap();
        stale.wake_by_ref();
        drop(stale);
    }

    struct SelfWaking(Arc<AtomicUsize>);

    impl Future for SelfWaking {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    #[test]
    fn self_waking_task_is_polled_once_per_batch() {
        let queue = TaskQueue::new();
        let polls = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        unsafe {
            let completed = completed.clone();
            queue.push(async move { completed.store(true, Ordering::Relaxed) });
            queue.push(SelfWaking(polls.clone()));
        }

        assert_eq!(poll(&queue), TaskPoll::Progress);
        assert!(completed.load(Ordering::Relaxed));
        assert_eq!(polls.load(Ordering::Relaxed), 1);
        assert_eq!(poll(&queue), TaskPoll::Pending);
        assert_eq!(polls.load(Ordering::Relaxed), 2);
    }

    struct ThreadWake {
        captured: Arc<StdMutex<Option<Waker>>>,
        ready: Arc<AtomicBool>,
    }

    impl Future for ThreadWake {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.ready.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                *self.captured.lock().unwrap() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    #[test]
    fn waker_may_wake_from_another_thread() {
        let queue = TaskQueue::new();
        let captured = Arc::new(StdMutex::new(None));
        let ready = Arc::new(AtomicBool::new(false));
        unsafe {
            queue.push(ThreadWake {
                captured: captured.clone(),
                ready: ready.clone(),
            });
        }
        assert_eq!(poll(&queue), TaskPoll::Pending);

        let waker = captured.lock().unwrap().take().unwrap();
        std::thread::spawn(move || {
            ready.store(true, Ordering::Release);
            waker.wake();
        })
        .join()
        .unwrap();

        assert_eq!(poll(&queue), TaskPoll::Empty);
    }
}
