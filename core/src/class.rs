use crate::{qjs, Ctx, Error, Object, Outlive, Result};
use std::{
    ffi::CString,
    marker::PhantomData,
    ptr::{self, NonNull},
};

mod id;
pub use id::ClassId;

mod cell;
pub use cell::{JsCell, Mutability, Readable, Writable};
mod ffi;

pub trait JsClass {
    /// The name the constructor has in javascript
    const NAME: &'static str;

    /// Can the type be mutated while a javascript value.
    type Mutable: Mutability;

    /// A unique id for the class.
    fn class_id() -> &'static ClassId;
}

pub struct Class<'js, C>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, C> Clone for Class<'js, C> {
    fn clone(&self) -> Self {
        Class(self.0.clone(), PhantomData)
    }
}

unsafe impl<'js, 't, C> Outlive<'t> for Class<'js, C> {
    type Target = Class<'t, C>;
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
    pub fn into_inner<'a>(&self) -> &'a JsCell<C> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// returns a pointer to the class object.
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
