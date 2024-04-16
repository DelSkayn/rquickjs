use std::{
    cell::Cell,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

use super::atomic_waker::AtomicWaker;

pub struct NodeHeader {
    next: AtomicPtr<NodeHeader>,
}

impl NodeHeader {
    pub fn new() -> NodeHeader {
        NodeHeader {
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

pub struct Queue {
    waker: AtomicWaker,
    head: AtomicPtr<NodeHeader>,
    tail: Cell<*const NodeHeader>,
    stub: NodeHeader,
}

unsafe impl Send for Queue {}
unsafe impl Sync for Queue {}

#[derive(Debug)]
pub enum Pop {
    Empty,
    Value(NonNull<NodeHeader>),
    Inconsistant,
}

/// Intrusive MPSC queue from 1024cores blog.
/// Similar to the one used int the FuturesUnordered implementation
impl Queue {
    pub fn new() -> Self {
        Queue {
            waker: AtomicWaker::new(),
            head: AtomicPtr::new(ptr::null_mut()),
            tail: Cell::new(ptr::null_mut()),
            stub: NodeHeader {
                next: AtomicPtr::new(ptr::null_mut()),
            },
        }
    }

    pub fn waker(&self) -> &AtomicWaker {
        &self.waker
    }

    pub unsafe fn init(self: Pin<&Self>) {
        let ptr = &self.stub as *const _ as *mut _;
        self.head.store(ptr, Ordering::Release);
        self.tail.set(ptr);
    }

    /// # Safety
    /// - node must be a valid pointer
    /// - Queue must have been properly initialized.
    pub unsafe fn push(self: Pin<&Self>, node: NonNull<NodeHeader>) {
        node.as_ref().next.store(ptr::null_mut(), Ordering::Release);

        let prev = self.get_ref().head.swap(node.as_ptr(), Ordering::AcqRel);

        (*prev).next.store(node.as_ptr(), Ordering::Release);
    }

    /// # Safety
    /// - Queue must have been properly initialized.
    /// - Can only be called from a single thread.
    pub unsafe fn pop(self: Pin<&Self>) -> Pop {
        let mut tail = self.tail.get();
        let mut next = (*tail).next.load(Ordering::Acquire);

        if std::ptr::eq(tail, &self.get_ref().stub) {
            if next.is_null() {
                return Pop::Empty;
            }

            self.tail.set(next);
            tail = next;
            next = (*next).next.load(std::sync::atomic::Ordering::Acquire);
        }

        if !next.is_null() {
            self.tail.set(next);
            return Pop::Value(NonNull::new_unchecked(tail as *mut NodeHeader));
        }

        let head = self.head.load(Ordering::Acquire);
        if !std::ptr::eq(head, tail) {
            return Pop::Inconsistant;
        }

        self.push(NonNull::from(&self.get_ref().stub));

        next = (*tail).next.load(Ordering::Acquire);

        if !next.is_null() {
            self.tail.set(next);
            return Pop::Value(NonNull::new_unchecked(tail as *mut NodeHeader));
        }

        Pop::Empty
    }
}
