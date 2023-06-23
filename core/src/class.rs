use crate::{qjs, Ctx, Error, Object, Outlive, Result};
use std::{
    ffi::CString,
    marker::PhantomData,
    ops::Deref,
    ptr::{self, NonNull},
};

mod id;
pub use id::ClassId;

mod cell;
pub use cell::{JsCell, Mutability, Readable, Writable};

pub use self::cell::{Borrow, BorrowError, BorrowMut};
mod ffi;

pub unsafe trait JsClass {
    /// The name the constructor has in javascript
    const NAME: &'static str;

    /// Can the type be mutated while a javascript value.
    type Mutable: Mutability;

    /// The class with any possible 'js lifetimes changed to 'a.
    type Outlive<'a>;

    /// A unique id for the class.
    fn class_id() -> &'static ClassId;

    /// The class prototype,
    fn prototype<'js>(ctx: Ctx<'js>) -> Result<Option<Object<'js>>>;
}

/// A object which is instance of a rust class.
pub struct Class<'js, C: JsClass>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, C: JsClass> Clone for Class<'js, C> {
    fn clone(&self) -> Self {
        Class(self.0.clone(), PhantomData)
    }
}

unsafe impl<'js, 't, C: JsClass> Outlive<'t> for Class<'js, C> {
    type Target = Class<'t, C>;
}

impl<'js, C: JsClass> Deref for Class<'js, C> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, C: JsClass> Class<'js, C> {
    /// Create a class from a rust object.
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        let val = unsafe {
            ctx.handle_exception(qjs::JS_NewObjectClass(
                ctx.as_ptr(),
                C::class_id().get() as i32,
            ))?
        };
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
            PhantomData,
        ))
    }

    /// Create a class from a rust object with a given prototype
    pub fn instance_proto(value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        let val = unsafe {
            proto.ctx.handle_exception(qjs::JS_NewObjectProtoClass(
                proto.ctx().as_ptr(),
                proto.0.as_js_value(),
                C::class_id().get(),
            ))?
        };
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(proto.ctx, val) },
            PhantomData,
        ))
    }

    /// Registers the class into the runtime.
    pub fn register(ctx: Ctx<'js>) -> Result<()> {
        let rt = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        let class_id = C::class_id().get();
        if 0 == unsafe { qjs::JS_IsRegisteredClass(rt, class_id) } {
            let class_name = CString::new(C::NAME).expect("class name has an internal null byte");
            let finalizer = if std::mem::needs_drop::<JsCell<C>>() {
                Some(ffi::finalizer::<C> as unsafe extern "C" fn(*mut qjs::JSRuntime, qjs::JSValue))
            } else {
                None
            };
            let class_def = qjs::JSClassDef {
                class_name: class_name.as_ptr(),
                finalizer,
                gc_mark: None,
                call: None,
                exotic: ptr::null_mut(),
            };
            if 0 != unsafe { qjs::JS_NewClass(rt, class_id, &class_def) } {
                return Err(Error::Unknown);
            }
        }
        Ok(())
    }

    /// Returns a reference to the underlying object.
    #[inline]
    pub fn as_class<'a>(&self) -> &'a JsCell<C> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// Borrow the rust class type.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class as a `Rc<RefCell<C>>` and borrowing functions similarly.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably
    #[inline]
    pub fn borrow<'a>(&'a self) -> Borrow<'a, C> {
        self.as_class().borrow()
    }

    /// Borrow the rust class type mutably.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class as a `Rc<RefCell<C>>` and borrowing functions similarly.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably or immutably, or the Class
    /// can't be borrowed mutably.
    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> BorrowMut<'a, C> {
        self.as_class().borrow_mut()
    }

    /// Try to borrow the rust class type.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class as a `Rc<RefCell<C>>` and borrowing functions similarly.
    ///
    /// This returns an error when the class is already borrowed mutably.
    #[inline]
    pub fn try_borrow<'a>(&'a self) -> Result<Borrow<'a, C>> {
        self.as_class().try_borrow().map_err(Error::from)
    }

    /// Try to borrow the rust class type mutably.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class as a `Rc<RefCell<C>>` and borrowing functions similarly.
    ///
    /// This returns an error when the class is already borrowed mutably, immutably or the class
    /// can't be borrowed mutably.
    #[inline]
    pub fn try_borrow_mut<'a>(&'a self) -> Result<BorrowMut<'a, C>> {
        self.as_class().try_borrow_mut().map_err(Error::from)
    }

    /// returns a pointer to the class object.
    #[inline]
    pub(crate) fn get_class_ptr(&self) -> NonNull<JsCell<C>> {
        let ptr = unsafe {
            qjs::JS_GetOpaque2(
                self.0.ctx.as_ptr(),
                self.0 .0.as_js_value(),
                C::class_id().get(),
            )
        };
        NonNull::new(ptr.cast()).expect("invalid class object, object didn't have opaque value")
    }
}
