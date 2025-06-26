use crate::{
    class::{self, ffi::VTable, JsClass},
    qjs, Ctx, Error, JsLifetime, Object, Value,
};

use super::{
    userdata::{UserDataGuard, UserDataMap},
    InterruptHandler, PromiseHook, PromiseHookType, RejectionTracker, UserDataError,
};
use alloc::boxed::Box;
use core::{
    any::{Any, TypeId},
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    ptr,
};

#[cfg(feature = "std")]
use std::collections::{hash_map::Entry, HashMap};

#[cfg(not(feature = "std"))]
use hashbrown::{hash_map::Entry, HashMap};

#[cfg(feature = "futures")]
use super::{schedular::SchedularPoll, spawner::Spawner};

#[cfg(feature = "futures")]
use core::{
    future::Future,
    task::{Context, Waker},
};

/// Opaque book keeping data for Rust.
pub(crate) struct Opaque<'js> {
    /// Used to carry a panic if a callback triggered one.
    panic: Cell<Option<Box<dyn Any + Send + 'static>>>,

    /// The user provided promise hook, if any.
    promise_hook: UnsafeCell<Option<PromiseHook>>,

    /// The user provided rejection tracker, if any.
    rejection_tracker: UnsafeCell<Option<RejectionTracker>>,

    /// The user provided interrupt handler, if any.
    interrupt_handler: UnsafeCell<Option<InterruptHandler>>,

    /// The class id for rust classes.
    class_id: qjs::JSClassID,
    /// The class id for rust classes which can be called.
    callable_class_id: qjs::JSClassID,

    prototypes: UnsafeCell<HashMap<TypeId, Option<Object<'js>>>>,

    userdata: UserDataMap,

    #[cfg(feature = "futures")]
    spawner: Option<UnsafeCell<Spawner>>,

    _marker: PhantomData<&'js ()>,
}

impl<'js> Opaque<'js> {
    pub fn new() -> Self {
        Opaque {
            panic: Cell::new(None),

            promise_hook: UnsafeCell::new(None),

            rejection_tracker: UnsafeCell::new(None),

            interrupt_handler: UnsafeCell::new(None),

            class_id: qjs::JS_INVALID_CLASS_ID,
            callable_class_id: qjs::JS_INVALID_CLASS_ID,

            prototypes: UnsafeCell::new(HashMap::new()),

            userdata: UserDataMap::default(),

            _marker: PhantomData,

            #[cfg(feature = "futures")]
            spawner: None,
        }
    }

    #[cfg(feature = "futures")]
    pub fn with_spawner() -> Self {
        let mut this = Opaque::new();
        this.spawner = Some(UnsafeCell::new(Spawner::new()));
        this
    }

    pub unsafe fn initialize(&mut self, rt: *mut qjs::JSRuntime) -> Result<(), Error> {
        qjs::JS_NewClassID(rt, (&mut self.class_id) as *mut qjs::JSClassID);
        qjs::JS_NewClassID(rt, (&mut self.callable_class_id) as *mut qjs::JSClassID);

        let class_def = qjs::JSClassDef {
            class_name: c"RustClass".as_ptr().cast(),
            finalizer: Some(class::ffi::class_finalizer),
            gc_mark: Some(class::ffi::class_trace),
            call: None,
            exotic: ptr::null_mut(),
        };

        if 0 != qjs::JS_NewClass(rt, self.class_id, &class_def) {
            return Err(Error::Unknown);
        }

        let class_def = qjs::JSClassDef {
            class_name: c"RustFunction".as_ptr().cast(),
            finalizer: Some(class::ffi::callable_finalizer),
            gc_mark: Some(class::ffi::callable_trace),
            call: Some(class::ffi::call),
            exotic: ptr::null_mut(),
        };

        if 0 != qjs::JS_NewClass(rt, self.callable_class_id, &class_def) {
            return Err(Error::Unknown);
        }

        Ok(())
    }

    pub unsafe fn from_runtime_ptr<'a>(rt: *mut qjs::JSRuntime) -> &'a Self {
        &*(qjs::JS_GetRuntimeOpaque(rt).cast::<Self>())
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
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        self.userdata.insert(data)
    }

    pub fn remove_userdata<U>(&self) -> Result<Option<Box<U>>, UserDataError<()>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        self.userdata.remove()
    }

    pub fn get_userdata<U>(&self) -> Option<UserDataGuard<U>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        self.userdata.get()
    }

    pub fn set_promise_hook(&self, promise_hook: Option<PromiseHook>) {
        unsafe { (*self.promise_hook.get()) = promise_hook }
    }

    pub fn run_promise_hook<'a>(
        &self,
        ctx: Ctx<'a>,
        type_: PromiseHookType,
        promise: Value<'a>,
        parent: Value<'a>,
    ) {
        unsafe { (*self.promise_hook.get()).as_mut().unwrap()(ctx, type_, promise, parent) }
    }

    pub fn set_rejection_tracker(&self, tracker: Option<RejectionTracker>) {
        unsafe { (*self.rejection_tracker.get()) = tracker }
    }

    pub fn run_rejection_tracker<'a>(
        &self,
        ctx: Ctx<'a>,
        promise: Value<'a>,
        reason: Value<'a>,
        is_handled: bool,
    ) {
        unsafe {
            (*self.rejection_tracker.get()).as_mut().unwrap()(ctx, promise, reason, is_handled)
        }
    }

    pub fn set_interrupt_handler(&self, interupt: Option<InterruptHandler>) {
        unsafe { (*self.interrupt_handler.get()) = interupt }
    }

    pub fn run_interrupt_handler(&self) -> bool {
        unsafe { (*self.interrupt_handler.get()).as_mut().unwrap()() }
    }

    #[allow(dead_code)] // not used in no_std
    pub fn set_panic(&self, panic: Box<dyn Any + Send + 'static>) {
        self.panic.set(Some(panic))
    }

    pub fn take_panic(&self) -> Option<Box<dyn Any + Send + 'static>> {
        self.panic.take()
    }

    pub fn get_class_id(&self) -> qjs::JSClassID {
        self.class_id
    }

    pub fn get_callable_id(&self) -> qjs::JSClassID {
        self.callable_class_id
    }

    pub fn get_or_insert_prototype<C: JsClass<'js>>(
        &self,
        ctx: &Ctx<'js>,
    ) -> Result<Option<Object<'js>>, Error> {
        unsafe {
            let vtable = VTable::get::<C>();
            let id = vtable.id();
            match (*self.prototypes.get()).entry(id) {
                Entry::Occupied(x) => Ok(x.get().clone()),
                Entry::Vacant(x) => {
                    let proto = C::prototype(ctx)?;
                    Ok(x.insert(proto).clone())
                }
            }
        }
    }

    /// Cleans up all the internal state.
    ///
    /// Called before dropping the runtime to ensure that we drop everything before freeing the
    /// runtime.
    pub fn clear(&mut self) {
        self.rejection_tracker.get_mut().take();
        self.interrupt_handler.get_mut().take();
        self.panic.take();
        self.prototypes.get_mut().clear();
        #[cfg(feature = "futures")]
        self.spawner.take();
        self.userdata.clear()
    }
}
