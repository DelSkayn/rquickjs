use rquickjs_sys::JSRuntime;
use tinyvec::TinyVec;

use crate::qjs;

use super::{
    userdata::{UserDataGuard, UserDataMap},
    InterruptHandler, UserData, UserDataError,
};
use std::{
    any::Any,
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
};

#[cfg(feature = "futures")]
use super::{schedular::SchedularPoll, spawner::Spawner};

#[cfg(feature = "futures")]
use std::{
    future::Future,
    task::{Context, Waker},
};

/// Opaque book keeping data for Rust.
pub(crate) struct Opaque<'js> {
    /// Used to carry a panic if a callback triggered one.
    panic: Cell<Option<Box<dyn Any + Send + 'static>>>,

    /// The user provided interrupt handler, if any.
    interrupt_handler: UnsafeCell<Option<InterruptHandler>>,

    class_ids: TinyVec<[qjs::JSClassID; 1024]>,

    userdata: UserDataMap,

    #[cfg(feature = "futures")]
    spawner: Option<UnsafeCell<Spawner>>,

    _marker: PhantomData<&'js ()>,
}

impl<'js> Opaque<'js> {
    pub fn new() -> Self {
        Opaque {
            panic: Cell::new(None),
            interrupt_handler: UnsafeCell::new(None),
            class_ids: TinyVec::from([0; 1024]),
            userdata: UserDataMap::default(),
            #[cfg(feature = "futures")]
            spawner: None,
            _marker: PhantomData,
        }
    }

    #[cfg(feature = "futures")]
    pub fn with_spawner() -> Self {
        Opaque {
            panic: Cell::new(None),
            interrupt_handler: UnsafeCell::new(None),
            class_ids: TinyVec::from([0; 1024]),
            userdata: UserDataMap::default(),
            #[cfg(feature = "futures")]
            spawner: Some(UnsafeCell::new(Spawner::new())),
            _marker: PhantomData,
        }
    }

    #[cfg(feature = "futures")]
    fn spawner(&self) -> &UnsafeCell<Spawner> {
        self.spawner
            .as_ref()
            .expect("tried to use async function in non async runtime")
    }

    #[cfg(feature = "futures")]
    pub unsafe fn push<F>(&self, f: F)
    where
        F: Future<Output = ()>,
    {
        (*self.spawner().get()).push(f)
    }

    #[cfg(feature = "futures")]
    pub fn listen(&self, wake: Waker) {
        unsafe { (*self.spawner().get()).listen(wake) };
    }

    #[cfg(feature = "futures")]
    pub fn spawner_is_empty(&self) -> bool {
        unsafe { (*self.spawner().get()).is_empty() }
    }

    #[cfg(feature = "futures")]
    pub fn poll(&self, cx: &mut Context) -> SchedularPoll {
        unsafe { (*self.spawner().get()).poll(cx) }
    }

    pub fn insert_userdata<U>(&self, data: U) -> Result<Option<Box<U>>, UserDataError<U>>
    where
        U: UserData<'js>,
    {
        self.userdata.insert(data)
    }

    pub fn remove_userdata<U>(&self) -> Result<Option<Box<U>>, UserDataError<()>>
    where
        U: UserData<'js>,
    {
        self.userdata.remove()
    }

    pub fn get_userdata<U: UserData<'js>>(&self) -> Option<UserDataGuard<U>> {
        self.userdata.get()
    }

    pub fn set_interrupt_handler(&self, interupt: Option<InterruptHandler>) {
        unsafe { (*self.interrupt_handler.get()) = interupt }
    }

    pub fn run_interrupt_handler(&self) -> bool {
        unsafe { (*self.interrupt_handler.get()).as_mut().unwrap()() }
    }

    pub fn set_panic(&self, panic: Box<dyn Any + Send + 'static>) {
        self.panic.set(Some(panic))
    }

    pub fn take_panic(&self) -> Option<Box<dyn Any + Send + 'static>> {
        self.panic.take()
    }

    pub(crate) fn register_class(&mut self, rt: *mut JSRuntime, type_id: u32) -> qjs::JSClassID {
        let type_id = type_id as usize;
        let old_capacity = self.class_ids.capacity();
        if type_id < old_capacity {
            let id = self.class_ids.get(type_id).unwrap_or(&0);
            if id != &0 {
                return *id;
            }
        } else {
            self.class_ids.reserve(1);
            let new_capacity = self.class_ids.capacity();
            for _ in old_capacity..new_capacity {
                self.class_ids.push(0);
            }
        }
        let mut id = 0;
        unsafe { qjs::JS_NewClassID(rt, &mut id) };
        self.class_ids[type_id] = id;
        id
    }
}
