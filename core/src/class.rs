//! JavaScript classes defined from Rust.

use crate::{
    function::Params,
    qjs::{self},
    value::Constructor,
    Ctx, Error, FromJs, IntoJs, JsLifetime, Object, Result, Value,
};
use alloc::boxed::Box;
use core::{hash::Hash, marker::PhantomData, mem, ops::Deref, ptr::NonNull};

mod cell;
mod trace;

#[doc(hidden)]
pub mod ffi;

pub use cell::{
    Borrow, BorrowMut, JsCell, Mutability, OwnedBorrow, OwnedBorrowMut, Readable, Writable,
};
use ffi::{ClassCell, VTable};
pub use trace::{Trace, Tracer};
#[doc(hidden)]
pub mod impl_;
pub mod inherits;

/// The trait which allows Rust types to be used from JavaScript.
pub trait JsClass<'js>: Trace<'js> + JsLifetime<'js> + Sized {
    /// The name the constructor has in JavaScript
    const NAME: &'static str;

    /// Is this class a function.
    const CALLABLE: bool = false;

    /// Can the type be mutated while a JavaScript value.
    ///
    /// This should either be [`Readable`] or [`Writable`].
    type Mutable: Mutability;

    /// Returns the parent class vtable if this class extends another class.
    fn parent_vtable() -> Option<&'static VTable> {
        None
    }

    /// Returns the class prototype,
    fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        Object::new(ctx.clone()).map(Some)
    }

    /// Returns a predefined constructor for this specific class type if there is one.
    fn constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>>;

    /// The function which will be called if [`Self::CALLABLE`] is true and an an object with this
    /// class is called as if it is a function.
    fn call<'a>(this: &JsCell<'js, Self>, params: Params<'a, 'js>) -> Result<Value<'js>> {
        let _ = this;
        Ok(Value::new_undefined(params.ctx().clone()))
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
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

unsafe impl<'js, C> JsLifetime<'js> for Class<'js, C>
where
    C: JsClass<'js> + JsLifetime<'js>,
    for<'to> C::Changed<'to>: JsClass<'to>,
{
    type Changed<'to> = Class<'to, C::Changed<'to>>;
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
        let id = unsafe {
            if C::CALLABLE {
                ctx.get_opaque().get_callable_id()
            } else {
                ctx.get_opaque().get_class_id()
            }
        };

        let prototype = Self::prototype(&ctx)?;

        let prototype = prototype.map(|x| x.as_js_value()).unwrap_or(qjs::JS_NULL);
        let val = unsafe {
            ctx.handle_exception(qjs::JS_NewObjectProtoClass(ctx.as_ptr(), prototype, id))?
        };

        let ptr = Box::into_raw(Box::new(ClassCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
            PhantomData,
        ))
    }

    /// Create a class from a Rust object with a given prototype.
    pub fn instance_proto(value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        let id = unsafe {
            if C::CALLABLE {
                proto.ctx().get_opaque().get_callable_id()
            } else {
                proto.ctx().get_opaque().get_class_id()
            }
        };

        let val = unsafe {
            proto.ctx.handle_exception(qjs::JS_NewObjectProtoClass(
                proto.ctx().as_ptr(),
                proto.0.as_js_value(),
                id,
            ))?
        };
        let ptr = Box::into_raw(Box::new(ClassCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(proto.ctx.clone(), val) },
            PhantomData,
        ))
    }

    /// Returns the prototype for the class.
    ///
    /// Returns `None` if the class is not yet registered or if the class doesn't have a prototype.
    pub fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        unsafe { ctx.get_opaque().get_or_insert_prototype::<C>(ctx) }
    }

    /// Returns the constructor for the current class using its definition.
    ///
    /// Returns `None` if the class is not yet registered or if the class doesn't have a constructor.
    pub fn constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        unsafe { ctx.get_opaque().get_or_insert_constructor::<C>(ctx) }
    }

    /// Returns the constructor for the current class using its definition.
    ///
    /// Returns `None` if the class is not yet registered or if the class doesn't have a constructor.
    pub fn create_constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        Self::constructor(ctx)
    }

    /// Defines the predefined constructor of this class, if there is one, onto the given object.
    pub fn define(object: &Object<'js>) -> Result<()> {
        if let Some(constructor) = Self::create_constructor(object.ctx())? {
            object.set(C::NAME, constructor)?;
        }
        Ok(())
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub(crate) fn get_class_cell<'a>(&self) -> &'a ClassCell<JsCell<'js, C>> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub fn get_cell<'a>(&self) -> &'a JsCell<'js, C> {
        &self.get_class_cell().data
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
    pub(crate) fn get_class_ptr(&self) -> NonNull<ClassCell<JsCell<'js, C>>> {
        let id = unsafe {
            if C::CALLABLE {
                self.ctx.get_opaque().get_callable_id()
            } else {
                self.ctx.get_opaque().get_class_id()
            }
        };

        let ptr = unsafe { qjs::JS_GetOpaque2(self.0.ctx.as_ptr(), self.0 .0.as_js_value(), id) };

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
    pub fn from_value(value: &Value<'js>) -> Result<Self> {
        if let Some(cls) = value.as_object().and_then(Self::from_object) {
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
    pub fn from_object(object: &Object<'js>) -> Option<Self> {
        object.into_class().ok()
    }
}

impl<'js> Object<'js> {
    /// Returns if the object is of a certain Rust class.
    pub fn instance_of<C: JsClass<'js>>(&self) -> bool {
        let id = unsafe {
            if C::CALLABLE {
                self.ctx.get_opaque().get_callable_id()
            } else {
                self.ctx.get_opaque().get_class_id()
            }
        };

        // This checks if the class is of the right class id.
        let Some(x) = NonNull::new(unsafe {
            qjs::JS_GetOpaque2(self.0.ctx.as_ptr(), self.0.as_js_value(), id)
        }) else {
            return false;
        };

        let v_table = unsafe { x.cast::<ClassCell<()>>().as_ref().v_table };

        // If the pointer is equal it must be of the right type, as the inclusion of a call to
        // generate a TypeId means that each type must have a unique v table.
        // however if it is not equal then it can still be the right type if the v_table is
        // duplicated, which is possible when compilation with multiple code-gen units.
        //
        // Doing check avoids a lookup and an dynamic function call in some cases.
        if core::ptr::eq(v_table, VTable::get::<C>()) {
            return true;
        }

        v_table.is_of_class::<C>()
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn into_class<C: JsClass<'js>>(&self) -> core::result::Result<Class<'js, C>, &Self> {
        if self.instance_of::<C>() {
            Ok(Class(self.clone(), PhantomData))
        } else {
            Err(self)
        }
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn as_class<C: JsClass<'js>>(&self) -> Option<&Class<'js, C>> {
        if self.instance_of::<C>() {
            // SAFETY:
            // Safe because class is a transparent wrapper
            unsafe { Some(mem::transmute::<&Object<'js>, &Class<'js, C>>(self)) }
        } else {
            None
        }
    }
}

impl<'js, C: JsClass<'js>> FromJs<'js> for Class<'js, C> {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Self::from_value(&value)
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
        class::{JsClass, Readable, Trace, Tracer, Writable},
        function::This,
        test_with,
        value::Constructor,
        CatchResultExt, Class, Context, FromJs, Function, IntoJs, JsLifetime, Object, Runtime,
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

        unsafe impl<'js> JsLifetime<'js> for Container<'js> {
            type Changed<'to> = Container<'to>;
        }

        impl<'js> JsClass<'js> for Container<'js> {
            const NAME: &'static str = "Container";

            type Mutable = Writable;

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

            assert!(cls.instance_of::<Container>());

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

    unsafe impl<'js> JsLifetime<'js> for Vec3 {
        type Changed<'to> = Vec3;
    }

    impl<'js> JsClass<'js> for Vec3 {
        const NAME: &'static str = "Vec3";

        type Mutable = Writable;

        fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::Object<'js>>> {
            let proto = Object::new(ctx.clone())?;
            let func = Function::new(ctx.clone(), |this: This<Vec3>, other: Vec3| this.add(other))?
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

    #[test]
    fn constructor() {
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
                .catch(&ctx)
                .unwrap();

            approx::assert_abs_diff_eq!(v.x, 5.0);
            approx::assert_abs_diff_eq!(v.y, 4.0);
            approx::assert_abs_diff_eq!(v.z, 11.0);

            let name: String = ctx.eval("new Vec3(1,2,3).constructor.name").unwrap();
            assert_eq!(name, Vec3::NAME);
        })
    }

    #[test]
    fn extend_class() {
        test_with(|ctx| {
            Class::<Vec3>::define(&ctx.globals()).unwrap();

            let v = ctx
                .eval::<Vec3, _>(
                    r"
                    class Vec4 extends Vec3 {
                        w = 0;
                        constructor(x,y,z,w){
                            super(x,y,z);
                            this.w
                        }
                    }

                    new Vec4(1,2,3,4);
                ",
                )
                .catch(&ctx)
                .unwrap();

            approx::assert_abs_diff_eq!(v.x, 1.0);
            approx::assert_abs_diff_eq!(v.y, 2.0);
            approx::assert_abs_diff_eq!(v.z, 3.0);
        })
    }

    #[test]
    fn get_prototype() {
        pub struct X;

        impl<'js> Trace<'js> for X {
            fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
        }

        unsafe impl<'js> JsLifetime<'js> for X {
            type Changed<'to> = X;
        }

        impl<'js> JsClass<'js> for X {
            const NAME: &'static str = "X";

            type Mutable = Readable;

            fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<Object<'js>>> {
                let object = Object::new(ctx.clone())?;
                object.set("foo", "bar")?;
                Ok(Some(object))
            }

            fn constructor(_ctx: &crate::Ctx<'js>) -> crate::Result<Option<Constructor<'js>>> {
                Ok(None)
            }
        }

        test_with(|ctx| {
            let proto = Class::<X>::prototype(&ctx).unwrap().unwrap();
            assert_eq!(proto.get::<_, String>("foo").unwrap(), "bar")
        })
    }

    #[test]
    fn generic_types() {
        pub struct DebugPrinter<D: std::fmt::Debug> {
            d: D,
        }

        impl<'js, D: std::fmt::Debug> Trace<'js> for DebugPrinter<D> {
            fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
        }

        unsafe impl<'js, D: std::fmt::Debug + 'static> JsLifetime<'js> for DebugPrinter<D> {
            type Changed<'to> = DebugPrinter<D>;
        }

        impl<'js, D: std::fmt::Debug + 'static> JsClass<'js> for DebugPrinter<D> {
            const NAME: &'static str = "DebugPrinter";

            type Mutable = Readable;

            fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<Object<'js>>> {
                let object = Object::new(ctx.clone())?;
                object.set(
                    "to_debug_string",
                    Function::new(
                        ctx.clone(),
                        |this: This<Class<DebugPrinter<D>>>| -> crate::Result<String> {
                            Ok(format!("{:?}", &this.0.borrow().d))
                        },
                    ),
                )?;
                Ok(Some(object))
            }

            fn constructor(_ctx: &crate::Ctx<'js>) -> crate::Result<Option<Constructor<'js>>> {
                Ok(None)
            }
        }

        test_with(|ctx| {
            let a = Class::instance(ctx.clone(), DebugPrinter { d: 42usize });
            let b = Class::instance(
                ctx.clone(),
                DebugPrinter {
                    d: "foo".to_string(),
                },
            );

            ctx.globals().set("a", a).unwrap();
            ctx.globals().set("b", b).unwrap();

            assert_eq!(
                ctx.eval::<String, _>(r#" a.to_debug_string() "#)
                    .catch(&ctx)
                    .unwrap(),
                "42"
            );
            assert_eq!(
                ctx.eval::<String, _>(r#" b.to_debug_string() "#)
                    .catch(&ctx)
                    .unwrap(),
                "\"foo\""
            );

            if ctx
                .globals()
                .get::<_, Class<DebugPrinter<String>>>("a")
                .is_ok()
            {
                panic!("Conversion should fail")
            }
            if ctx
                .globals()
                .get::<_, Class<DebugPrinter<usize>>>("b")
                .is_ok()
            {
                panic!("Conversion should fail")
            }

            ctx.globals()
                .get::<_, Class<DebugPrinter<usize>>>("a")
                .unwrap();
            ctx.globals()
                .get::<_, Class<DebugPrinter<String>>>("b")
                .unwrap();
        })
    }
}
