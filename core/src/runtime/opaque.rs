use crate::{
    class::{self, ffi::VTable, JsClass},
    qjs, Ctx, Error, Object,
};

use super::{
    userdata::{UserDataGuard, UserDataMap},
    InterruptHandler, UserData, UserDataError,
};
use std::{
    any::Any,
    cell::{Cell, UnsafeCell},
    collections::{hash_map::Entry, HashMap},
    marker::PhantomData,
    ptr,
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

    /// The class id for rust classes.
    class_id: qjs::JSClassID,
    /// The class id for rust classes which can be called.
    callable_class_id: qjs::JSClassID,

    prototypes: UnsafeCell<HashMap<*const VTable, Option<Object<'js>>>>,

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
        qjs::JS_NewClassID((&mut self.class_id) as *mut qjs::JSClassID);
        qjs::JS_NewClassID((&mut self.callable_class_id) as *mut qjs::JSClassID);

        let class_def = qjs::JSClassDef {
            class_name: b"RustClass\0".as_ptr().cast(),
            finalizer: Some(class::ffi::class_finalizer),
            gc_mark: Some(class::ffi::class_trace),
            call: None,
            exotic: ptr::null_mut(),
        };

        if 0 != qjs::JS_NewClass(rt, self.class_id, &class_def) {
            return Err(Error::Unknown);
        }

        let class_def = qjs::JSClassDef {
            class_name: b"RustFunction\0".as_ptr().cast(),
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
            match (*self.prototypes.get()).entry(vtable) {
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
        self.interrupt_handler.get_mut().take();
        self.panic.take();
        self.prototypes.get_mut().clear();
        #[cfg(feature = "futures")]
        self.spawner.take();
        self.userdata.clear()
    }
}
