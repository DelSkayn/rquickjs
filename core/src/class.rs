use crate::{
    function::StaticJsFn, module::Exports, qjs, value::Constructor, Ctx, Error, FromJs, IntoJs,
    Object, Outlive, Result, Value,
};
use std::{
    ffi::CString,
    marker::PhantomData,
    ops::Deref,
    ptr::{self, NonNull},
};

mod id;
pub use id::ClassId;

mod cell;
pub use cell::{
    Borrow, BorrowMut, JsCell, Mutability, OwnedBorrow, OwnedBorrowMut, Readable, Writable,
};
mod ffi;
mod trace;
pub use trace::{Trace, Tracer};
#[doc(hidden)]
pub mod impl_;

pub trait JsClass<'js>: Trace<'js> {
    /// The name the constructor has in javascript
    const NAME: &'static str;

    /// Can the type be mutated while a javascript value.
    type Mutable: Mutability;

    /// A unique id for the class.
    fn class_id() -> &'static ClassId;

    /// Returns the class prototype,
    fn prototype(ctx: Ctx<'js>) -> Result<Option<Object<'js>>>;

    /// Returns a predefined constructor for this specific class type if there is one.
    fn constructor(ctx: Ctx<'js>) -> Result<Option<Constructor<'js>>>;

    /// A possible call function.
    ///
    /// Returning a function from this method makes any objects with this class callable as if it
    /// is a function object..
    fn function() -> Option<StaticJsFn> {
        None
    }
}

/// A object which is instance of a rust class.
pub struct Class<'js, C: JsClass<'js>>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, C: JsClass<'js>> Clone for Class<'js, C> {
    fn clone(&self) -> Self {
        Class(self.0.clone(), PhantomData)
    }
}

unsafe impl<'js, C> Outlive<'js> for Class<'js, C>
where
    C: JsClass<'js> + Outlive<'js>,
    for<'to> C::Target<'to>: JsClass<'to>,
{
    type Target<'to> = Class<'to, C::Target<'to>>;
}

impl<'js, C: JsClass<'js>> Deref for Class<'js, C> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, C: JsClass<'js>> Class<'js, C> {
    /// Create a class from a rust object.
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        if !Self::is_registered(ctx) {
            Self::register(ctx)?;
        }

        let val = unsafe {
            ctx.handle_exception(qjs::JS_NewObjectClass(
                ctx.as_ptr(),
                C::class_id().get() as i32,
            ))?
        };
        let ptr: *mut JsCell<'js, C> = Box::into_raw(Box::new(JsCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
            PhantomData,
        ))
    }

    /// Create a class from a rust object with a given prototype
    pub fn instance_proto(value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        if !Self::is_registered(proto.ctx()) {
            Self::register(proto.ctx())?;
        }
        let val = unsafe {
            proto.ctx.handle_exception(qjs::JS_NewObjectProtoClass(
                proto.ctx().as_ptr(),
                proto.0.as_js_value(),
                C::class_id().get(),
            ))?
        };
        let ptr: *mut JsCell<'js, C> = Box::into_raw(Box::new(JsCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(proto.ctx, val) },
            PhantomData,
        ))
    }

    /// Returns the prototype for the class.
    ///
    /// Returns None if the class is not yet registered or if the class doesn't have a prototype
    pub fn prototype(ctx: Ctx<'js>) -> Option<Object<'js>> {
        if !Self::is_registered(ctx) {
            return None;
        }
        let proto = unsafe {
            let proto = qjs::JS_GetClassProto(ctx.as_ptr(), C::class_id().get());
            Value::from_js_value(ctx, proto)
        };
        if proto.is_null() {
            return None;
        }
        Some(
            proto
                .into_object()
                .expect("class prototype wasn't an object"),
        )
    }

    pub fn create_constructor(ctx: Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        Self::register(ctx)?;
        C::constructor(ctx)
    }

    /// Defines the predefined constructor of this class, if there is one, onto the given object.
    pub fn define(object: Object<'js>) -> Result<()> {
        if let Some(constructor) = Self::create_constructor(object.ctx())? {
            object.set(C::NAME, constructor)?;
        }
        Ok(())
    }

    /// Returns if the class is registered in the runtime.
    #[inline]
    pub fn is_registered(ctx: Ctx<'js>) -> bool {
        let rt = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        let class_id = C::class_id().get();
        0 != unsafe { qjs::JS_IsRegisteredClass(rt, class_id) }
    }

    /// Registers the class `C` into the runtime.
    ///
    /// It is required to call this function on every context in which the class is used before using the class.
    /// Otherwise the class
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
            let call = C::function().map(|x| x.0);
            let class_def = qjs::JSClassDef {
                class_name: class_name.as_ptr(),
                finalizer,
                gc_mark: None,
                call,
                exotic: ptr::null_mut(),
            };
            if 0 != unsafe { qjs::JS_NewClass(rt, class_id, &class_def) } {
                return Err(Error::Unknown);
            }
        }

        if unsafe {
            qjs::JS_VALUE_GET_TAG(qjs::JS_GetClassProto(ctx.as_ptr(), class_id)) == qjs::JS_TAG_NULL
        } {
            if let Some(proto) = C::prototype(ctx)? {
                let val = proto.into_value().into_js_value();
                unsafe { qjs::JS_SetClassProto(ctx.as_ptr(), class_id, val) }
            }
        }

        Ok(())
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub fn as_class<'a>(&self) -> &'a JsCell<'js, C> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// Borrow the rust class type.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably
    #[inline]
    pub fn borrow<'a>(&'a self) -> Borrow<'a, 'js, C> {
        self.as_class().borrow()
    }

    /// Borrow the rust class type mutably.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably or immutably, or the Class
    /// can't be borrowed mutably.
    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> BorrowMut<'a, 'js, C> {
        self.as_class().borrow_mut()
    }

    /// Try to borrow the rust class type.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably.
    #[inline]
    pub fn try_borrow<'a>(&'a self) -> Result<Borrow<'a, 'js, C>> {
        self.as_class().try_borrow().map_err(Error::ClassBorrow)
    }

    /// Try to borrow the rust class type mutably.
    ///
    /// Javascript classes behave similar to [`Rc`](std::rc::Rc) in rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably, immutably or the class
    /// can't be borrowed mutably.
    #[inline]
    pub fn try_borrow_mut<'a>(&'a self) -> Result<BorrowMut<'a, 'js, C>> {
        self.as_class().try_borrow_mut().map_err(Error::ClassBorrow)
    }

    /// returns a pointer to the class object.
    #[inline]
    pub(crate) fn get_class_ptr(&self) -> NonNull<JsCell<'js, C>> {
        let ptr = unsafe {
            qjs::JS_GetOpaque2(
                self.0.ctx.as_ptr(),
                self.0 .0.as_js_value(),
                C::class_id().get(),
            )
        };
        NonNull::new(ptr.cast()).expect("invalid class object, object didn't have opaque value")
    }

    /// Turns the class back into a generic object.
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        self.0
    }

    /// Converts a generic object into a class if the object is of the right class.
    pub fn from_object(object: Object<'js>) -> Option<Self> {
        object.into_class().ok()
    }
}

impl<'js> Object<'js> {
    /// Returns if the object is of a certain rust class.
    pub fn is_class<C: JsClass<'js>>(&self) -> bool {
        if !Class::<C>::is_registered(self.ctx) {
            return false;
        }

        let p = unsafe {
            qjs::JS_GetOpaque2(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                C::class_id().get(),
            )
        };
        !p.is_null()
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn into_class<C: JsClass<'js>>(self) -> std::result::Result<Class<'js, C>, Self> {
        if self.is_class::<C>() {
            Ok(Class(self, PhantomData))
        } else {
            Err(self)
        }
    }
}

impl<'js, C: JsClass<'js>> FromJs<'js> for Class<'js, C> {
    fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        if let Some(cls) = value.clone().into_object().and_then(Self::from_object) {
            return Ok(cls);
        }
        Err(Error::FromJs {
            from: value.type_name(),
            to: C::NAME,
            message: None,
        })
    }
}

impl<'js, C: JsClass<'js>> IntoJs<'js> for Class<'js, C> {
    fn into_js(self, _ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.0 .0)
    }
}
