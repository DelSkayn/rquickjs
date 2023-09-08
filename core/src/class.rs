//! JavaScript classes defined from Rust.

use crate::{
    function::StaticJsFn, qjs, value::Constructor, Ctx, Error, FromJs, IntoJs, Object, Outlive,
    Result, Value,
};
use std::{
    ffi::CString,
    hash::Hash,
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
use rquickjs_sys::JS_VALUE_GET_TAG;
pub use trace::{Trace, Tracer};
#[doc(hidden)]
pub mod impl_;

pub trait JsClass<'js>: Trace<'js> {
    /// The name the constructor has in JavaScript
    const NAME: &'static str;

    /// Can the type be mutated while a JavaScript value.
    type Mutable: Mutability;

    /// A unique id for the class.
    fn class_id() -> &'static ClassId;

    /// Returns the class prototype,
    fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>>;

    /// Returns a predefined constructor for this specific class type if there is one.
    fn constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>>;

    /// A possible call function.
    ///
    /// Returning a function from this method makes any objects with this class callable as if it
    /// is a function object..
    fn function() -> Option<StaticJsFn> {
        None
    }
}

/// A object which is instance of a Rust class.
#[repr(transparent)]
pub struct Class<'js, C: JsClass<'js>>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, C: JsClass<'js>> Clone for Class<'js, C> {
    fn clone(&self) -> Self {
        Class(self.0.clone(), PhantomData)
    }
}

impl<'js, C: JsClass<'js>> PartialEq for Class<'js, C> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'js, C: JsClass<'js>> Eq for Class<'js, C> {}

impl<'js, C: JsClass<'js>> Hash for Class<'js, C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
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
    /// Create a class from a Rust object.
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        if !Self::is_registered(&ctx) {
            Self::register(&ctx)?;
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

    /// Create a class from a Rust object with a given prototype
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
            unsafe { Object::from_js_value(proto.ctx.clone(), val) },
            PhantomData,
        ))
    }

    /// Returns the prototype for the class.
    ///
    /// Returns `None` if the class is not yet registered or if the class doesn't have a prototype.
    pub fn prototype(ctx: Ctx<'js>) -> Option<Object<'js>> {
        if !Self::is_registered(&ctx) {
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

    /// Create a constructor for the current class using its definition.
    pub fn create_constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        Self::register(ctx)?;
        C::constructor(ctx)
    }

    /// Defines the predefined constructor of this class, if there is one, onto the given object.
    pub fn define(object: &Object<'js>) -> Result<()> {
        if let Some(constructor) = Self::create_constructor(object.ctx())? {
            object.set(C::NAME, constructor)?;
        }
        Ok(())
    }

    /// Returns if the class is registered in the runtime.
    #[inline]
    pub fn is_registered(ctx: &Ctx<'js>) -> bool {
        let rt = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        let class_id = C::class_id().get();
        0 != unsafe { qjs::JS_IsRegisteredClass(rt, class_id) }
    }

    /// Registers the class `C` into the runtime.
    ///
    /// It is required to call this function on every context in which the class is used before using the class.
    /// Otherwise the class.
    ///
    /// It is fine to call this function multiple times, even on the same context. The class and
    /// its prototype will only be registered once.
    pub fn register(ctx: &Ctx<'js>) -> Result<()> {
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
                gc_mark: Some(ffi::trace::<C>),
                call,
                exotic: ptr::null_mut(),
            };
            if 0 != unsafe { qjs::JS_NewClass(rt, class_id, &class_def) } {
                return Err(Error::Unknown);
            }
        }

        let proto_val = unsafe { qjs::JS_GetClassProto(ctx.as_ptr(), class_id) };
        if unsafe { JS_VALUE_GET_TAG(proto_val) == qjs::JS_TAG_NULL } {
            if let Some(proto) = C::prototype(ctx)? {
                let val = proto.into_value().into_js_value();
                unsafe { qjs::JS_SetClassProto(ctx.as_ptr(), class_id, val) }
            }
        } else {
            unsafe { qjs::JS_FreeValue(ctx.as_ptr(), proto_val) }
        }

        Ok(())
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub fn get_cell<'a>(&self) -> &'a JsCell<'js, C> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// Borrow the Rust class type.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably.
    #[inline]
    pub fn borrow<'a>(&'a self) -> Borrow<'a, 'js, C> {
        self.get_cell().borrow()
    }

    /// Borrow the Rust class type mutably.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably or immutably, or the Class
    /// can't be borrowed mutably.
    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> BorrowMut<'a, 'js, C> {
        self.get_cell().borrow_mut()
    }

    /// Try to borrow the Rust class type.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably.
    #[inline]
    pub fn try_borrow<'a>(&'a self) -> Result<Borrow<'a, 'js, C>> {
        self.get_cell().try_borrow().map_err(Error::ClassBorrow)
    }

    /// Try to borrow the Rust class type mutably.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably, immutably or the class
    /// can't be borrowed mutably.
    #[inline]
    pub fn try_borrow_mut<'a>(&'a self) -> Result<BorrowMut<'a, 'js, C>> {
        self.get_cell().try_borrow_mut().map_err(Error::ClassBorrow)
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
    pub fn into_inner(self) -> Object<'js> {
        self.0
    }

    /// Turns the class back into a generic object.
    #[inline]
    pub fn as_inner(&self) -> &Object<'js> {
        &self.0
    }

    /// Convert from value.
    #[inline]
    pub fn from_value(value: Value<'js>) -> Result<Self> {
        if let Some(cls) = value.clone().into_object().and_then(Self::from_object) {
            return Ok(cls);
        }
        Err(Error::FromJs {
            from: value.type_name(),
            to: C::NAME,
            message: None,
        })
    }

    /// Turn the class into a value.
    #[inline]
    pub fn into_value(self) -> Value<'js> {
        self.0.into_value()
    }

    /// Converts a generic object into a class if the object is of the right class.
    #[inline]
    pub fn from_object(object: Object<'js>) -> Option<Self> {
        object.into_class().ok()
    }
}

impl<'js> Object<'js> {
    /// Returns if the object is of a certain Rust class.
    pub fn instance_of<C: JsClass<'js>>(&self) -> bool {
        if !Class::<C>::is_registered(&self.ctx) {
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
        if self.instance_of::<C>() {
            Ok(Class(self, PhantomData))
        } else {
            Err(self)
        }
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn as_class<C: JsClass<'js>>(&self) -> Option<&Class<'js, C>> {
        if self.instance_of::<C>() {
            // SAFETY:
            // Safe because class is a transparent wrapper
            unsafe { Some(std::mem::transmute::<&Object<'js>, &Class<'js, C>>(self)) }
        } else {
            None
        }
    }
}

impl<'js, C: JsClass<'js>> FromJs<'js> for Class<'js, C> {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
    fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.0 .0)
    }
}

#[cfg(test)]
mod test {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use crate::{
        class::{ClassId, JsClass, Readable, Trace, Tracer, Writable},
        function::This,
        test_with,
        value::Constructor,
        Class, Context, FromJs, Function, IntoJs, Object, Runtime,
    };

    /// Test circular references.
    #[test]
    fn trace() {
        pub struct Container<'js> {
            inner: Vec<Class<'js, Container<'js>>>,
            test: Arc<AtomicBool>,
        }

        impl<'js> Drop for Container<'js> {
            fn drop(&mut self) {
                self.test.store(true, Ordering::SeqCst);
            }
        }

        impl<'js> Trace<'js> for Container<'js> {
            fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
                self.inner.iter().for_each(|x| x.trace(tracer))
            }
        }

        impl<'js> JsClass<'js> for Container<'js> {
            const NAME: &'static str = "Container";

            type Mutable = Writable;

            fn class_id() -> &'static crate::class::ClassId {
                static ID: ClassId = ClassId::new();
                &ID
            }

            fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::Object<'js>>> {
                Ok(Some(Object::new(ctx.clone())?))
            }

            fn constructor(
                _ctx: &crate::Ctx<'js>,
            ) -> crate::Result<Option<crate::value::Constructor<'js>>> {
                Ok(None)
            }
        }

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        let drop_test = Arc::new(AtomicBool::new(false));

        ctx.with(|ctx| {
            let cls = Class::instance(
                ctx.clone(),
                Container {
                    inner: Vec::new(),
                    test: drop_test.clone(),
                },
            )
            .unwrap();
            let cls_clone = cls.clone();
            cls.borrow_mut().inner.push(cls_clone);
        });
        rt.run_gc();
        assert!(drop_test.load(Ordering::SeqCst));
        ctx.with(|ctx| {
            let cls = Class::instance(
                ctx.clone(),
                Container {
                    inner: Vec::new(),
                    test: drop_test.clone(),
                },
            )
            .unwrap();
            let cls_clone = cls.clone();
            cls.borrow_mut().inner.push(cls_clone);
            ctx.globals().set("t", cls).unwrap();
        });
    }

    #[test]
    fn constructor() {
        #[derive(Clone, Copy)]
        pub struct Vec3 {
            x: f32,
            y: f32,
            z: f32,
        }

        impl Vec3 {
            pub fn new(x: f32, y: f32, z: f32) -> Self {
                Vec3 { x, y, z }
            }

            pub fn add(self, v: Vec3) -> Self {
                Vec3 {
                    x: self.x + v.x,
                    y: self.y + v.y,
                    z: self.z + v.z,
                }
            }
        }

        impl<'js> Trace<'js> for Vec3 {
            fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
        }

        impl<'js> FromJs<'js> for Vec3 {
            fn from_js(ctx: &crate::Ctx<'js>, value: crate::Value<'js>) -> crate::Result<Self> {
                Ok(*Class::<Vec3>::from_js(ctx, value)?.try_borrow()?)
            }
        }

        impl<'js> IntoJs<'js> for Vec3 {
            fn into_js(self, ctx: &crate::Ctx<'js>) -> crate::Result<crate::Value<'js>> {
                Class::instance(ctx.clone(), self).into_js(ctx)
            }
        }

        impl<'js> JsClass<'js> for Vec3 {
            const NAME: &'static str = "Vec3";

            type Mutable = Writable;

            fn class_id() -> &'static crate::class::ClassId {
                static ID: ClassId = ClassId::new();
                &ID
            }

            fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::Object<'js>>> {
                let proto = Object::new(ctx.clone())?;
                let func =
                    Function::new(ctx.clone(), |this: This<Vec3>, other: Vec3| this.add(other))?
                        .with_name("add")?;

                proto.set("add", func)?;
                Ok(Some(proto))
            }

            fn constructor(
                ctx: &crate::Ctx<'js>,
            ) -> crate::Result<Option<crate::value::Constructor<'js>>> {
                let constr =
                    Constructor::new_class::<Vec3, _, _>(ctx.clone(), |x: f32, y: f32, z: f32| {
                        Vec3::new(x, y, z)
                    })?;

                Ok(Some(constr))
            }
        }

        test_with(|ctx| {
            Class::<Vec3>::define(&ctx.globals()).unwrap();

            let v = ctx
                .eval::<Vec3, _>(
                    r"
                let a = new Vec3(1,2,3);
                let b = new Vec3(4,2,8);
                a.add(b)
            ",
                )
                .unwrap();

            approx::assert_abs_diff_eq!(v.x, 5.0);
            approx::assert_abs_diff_eq!(v.y, 4.0);
            approx::assert_abs_diff_eq!(v.z, 11.0);
        })
    }

    #[test]
    fn register_twice() {
        pub struct X;

        impl<'js> Trace<'js> for X {
            fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
        }

        impl<'js> JsClass<'js> for X {
            const NAME: &'static str = "X";

            type Mutable = Readable;

            fn class_id() -> &'static ClassId {
                static ID: ClassId = ClassId::new();
                &ID
            }

            fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<Object<'js>>> {
                Object::new(ctx.clone()).map(Some)
            }

            fn constructor(_ctx: &crate::Ctx<'js>) -> crate::Result<Option<Constructor<'js>>> {
                Ok(None)
            }
        }

        test_with(|ctx| {
            Class::<X>::register(&ctx).unwrap();
            Class::<X>::register(&ctx).unwrap();
        })
    }
}
