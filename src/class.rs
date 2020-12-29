use crate::{
    qjs, value, AsJsValueRef, Ctx, Error, FromJs, IntoJs, JsRef, Object, Outlive, Persistent,
    Result, Value,
};
use std::{
    ffi::CString,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

/// The type of identifier of class
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct ClassId(pub(crate) qjs::JSClassID);

impl ClassId {
    pub const fn new() -> Self {
        Self(0)
    }
}

impl AsRef<qjs::JSClassID> for ClassId {
    fn as_ref(&self) -> &qjs::JSClassID {
        &self.0
    }
}

impl AsMut<qjs::JSClassID> for ClassId {
    fn as_mut(&mut self) -> &mut qjs::JSClassID {
        &mut self.0
    }
}

impl Deref for ClassId {
    type Target = qjs::JSClassID;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ClassId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The trait which class should define
///
/// ```
/// # use rquickjs::{ClassId, ClassDef, Ctx, Object, Result};
/// struct MyClass;
///
/// impl ClassDef for MyClass {
///     const CLASS_NAME: &'static str = "MyClass";
///     fn class_id() -> &'static mut ClassId {
///         static mut CLASS_ID: ClassId = ClassId::new();
///         unsafe { &mut CLASS_ID }
///     }
///     fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
///         Ok(())
///     }
/// }
/// ```
///
/// # Features
/// This trait is only available if the `classes` feature is enabled.
///
pub trait ClassDef {
    /// The name of a class
    const CLASS_NAME: &'static str;

    /// The reference to class identifier
    fn class_id() -> &'static mut ClassId;

    /// The class has prototype
    const HAS_PROTO: bool = false;

    /// The prototype initializer method
    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    /// The class has internal references to JS values
    ///
    /// Needed for correct garbage collection
    const HAS_REFS: bool = false;

    /// Mark internal references to JS values
    ///
    /// Should be implemented to work with garbage collector
    fn mark_refs(&self, _marker: &RefsMarker) {}
}

/// The class object interface
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
///
/// FIXME: Maybe it should be private.
pub struct Class<'js, C>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, 't, C> Outlive<'t> for Class<'js, C> {
    type Target = Class<'t, C>;
}

impl<'js, C> AsJsValueRef<'js> for Class<'js, C> {
    fn as_js_value_ref(&self) -> qjs::JSValue {
        self.0.as_js_value_ref()
    }
}

impl<'js, C> Deref for Class<'js, C> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, C> AsRef<C> for Class<'js, C>
where
    C: ClassDef,
{
    fn as_ref(&self) -> &C {
        let obj = &self.0;
        Class::<C>::try_ref(obj.0.ctx, obj).unwrap()
    }
}

impl<'js, C> AsMut<C> for Class<'js, C>
where
    C: ClassDef,
{
    fn as_mut(&mut self) -> &mut C {
        let obj = &self.0;
        Class::<C>::try_mut(obj.0.ctx, obj).unwrap()
    }
}

impl<'js, C> Class<'js, C>
where
    C: ClassDef,
{
    /// Wrap constructor of class
    pub fn constructor<F>(func: F) -> Constructor<C, F> {
        Constructor(func, PhantomData)
    }

    /// Instantiate the object of class
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        let class_id = *C::class_id().as_ref();
        let val = unsafe {
            value::handle_exception(ctx, qjs::JS_NewObjectClass(ctx.ctx, class_id as _))
        }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(Self(
            Object(unsafe { JsRef::from_js_value(ctx, val) }),
            PhantomData,
        ))
    }

    /// Instantiate the object of class with given prototype
    pub fn instance_proto(ctx: Ctx<'js>, value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        let class_id = *C::class_id().as_ref();
        let val = unsafe {
            value::handle_exception(
                ctx,
                qjs::JS_NewObjectProtoClass(ctx.ctx, proto.0.as_js_value(), class_id as _),
            )
        }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(Self(
            Object(unsafe { JsRef::from_js_value(ctx, val) }),
            PhantomData,
        ))
    }

    /// Get reference from object
    pub fn try_ref<'r>(ctx: Ctx<'js>, value: &Object<'js>) -> Result<&'r C> {
        Ok(unsafe { &*Self::try_ptr(ctx.ctx, value.0.as_js_value())? })
    }

    /// Get mutable reference from object
    pub fn try_mut<'r>(ctx: Ctx<'js>, value: &Object<'js>) -> Result<&'r mut C> {
        Ok(unsafe { &mut *Self::try_ptr(ctx.ctx, value.0.as_js_value())? })
    }

    /// Get instance pointer from object
    unsafe fn try_ptr(ctx: *mut qjs::JSContext, value: qjs::JSValue) -> Result<*mut C> {
        let class_id = *C::class_id().as_ref();
        let ptr = qjs::JS_GetOpaque2(ctx, value, class_id) as *mut C;
        if ptr.is_null() {
            return Err(Error::FromJs {
                from: "object",
                to: C::CLASS_NAME,
                message: None,
            });
        }
        Ok(ptr)
    }

    /// Register the class
    pub fn register(ctx: Ctx<'js>) -> Result<()> {
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };
        let class_id = unsafe { qjs::JS_NewClassID(C::class_id().as_mut()) };
        let class_name = CString::new(C::CLASS_NAME)?;
        if 0 == unsafe { qjs::JS_IsRegisteredClass(rt, class_id) } {
            let class_def = qjs::JSClassDef {
                class_name: class_name.as_ptr(),
                finalizer: Some(Self::finalizer),
                gc_mark: if C::HAS_REFS {
                    Some(Self::gc_mark)
                } else {
                    None
                },
                call: None, //Some(Self::call),
                exotic: ptr::null_mut(),
            };
            if 0 != unsafe { qjs::JS_NewClass(rt, class_id, &class_def) } {
                return Err(Error::Unknown);
            }

            if C::HAS_PROTO {
                let proto = Object::new(ctx)?;
                C::init_proto(ctx, &proto)?;

                unsafe { qjs::JS_SetClassProto(ctx.ctx, class_id, proto.0.into_js_value()) }
            }
        }

        Ok(())
    }

    /// Register the class using raw context
    pub unsafe fn register_raw(ctx: *mut qjs::JSContext) {
        Self::register(Ctx::from_ptr(ctx)).unwrap()
    }

    /// Get the own prototype object of a class
    pub fn prototype(ctx: Ctx<'js>) -> Result<Object<'js>> {
        let class_id = *C::class_id().as_ref();
        Ok(Object(unsafe {
            JsRef::from_js_value(ctx, qjs::JS_GetClassProto(ctx.ctx, class_id))
        }))
    }

    unsafe extern "C" fn gc_mark(
        rt: *mut qjs::JSRuntime,
        val: qjs::JSValue,
        mark_func: qjs::JS_MarkFunc,
    ) {
        let class_id = *C::class_id().as_ref();
        let ptr = qjs::JS_GetOpaque(val, class_id) as *mut C;
        debug_assert!(!ptr.is_null());
        let inst = &mut *ptr;
        let marker = RefsMarker { rt, mark_func };
        inst.mark_refs(&marker);
    }

    unsafe extern "C" fn finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let class_id = *C::class_id().as_ref();
        let ptr = qjs::JS_GetOpaque(val, class_id) as *mut C;
        debug_assert!(!ptr.is_null());
        let inst = Box::from_raw(ptr);
        qjs::JS_FreeValueRT(rt, val);
        mem::drop(inst);
    }
}

#[derive(Clone, Copy)]
pub struct RefsMarker {
    rt: *mut qjs::JSRuntime,
    mark_func: qjs::JS_MarkFunc,
}

impl RefsMarker {
    pub fn mark<'js, T>(&self, value: &Persistent<T>) {
        let val = value.value.get();
        if unsafe { qjs::JS_VALUE_HAS_REF_COUNT(val) } {
            unsafe { qjs::JS_MarkValue(self.rt, val, self.mark_func) };
            if 0 == unsafe { qjs::JS_ValueRefCount(val) } {
                value.value.set(qjs::JS_UNDEFINED);
            }
        }
    }
}

impl<'js, C> IntoJs<'js> for Class<'js, C>
where
    C: ClassDef,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.0.into_js(ctx)
    }
}

impl<'js, C> FromJs<'js> for Class<'js, C>
where
    C: ClassDef,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let value = Object::from_js(ctx, value)?;
        let _ = Class::<C>::try_ref(ctx, &value)?;
        Ok(Class(value, PhantomData))
    }
}

impl<'js, C> IntoJs<'js> for C
where
    C: ClassDef,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Class::<C>::instance(ctx, self).map(|Class(obj, _)| Value::Object(obj))
    }
}

impl<'js, C> FromJs<'js> for &C
where
    C: ClassDef,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let value = Object::from_js(ctx, value)?;
        Class::<C>::try_ref(ctx, &value)
    }
}

impl<'js, C> FromJs<'js> for &mut C
where
    C: ClassDef,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let value = Object::from_js(ctx, value)?;
        Class::<C>::try_mut(ctx, &value)
    }
}

/// The wrapper for constructor function
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
#[repr(transparent)]
pub struct Constructor<C, F>(F, PhantomData<C>);

impl<C, F> AsRef<F> for Constructor<C, F> {
    fn as_ref(&self) -> &F {
        &self.0
    }
}

impl<C, F> Deref for Constructor<C, F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The prototype setting wrapper
///
/// This wrapper helps instantiate a class with desired prototype
/// which is quite useful with constructors because allows class to be inheritable.
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
pub struct WithProto<'js, C>(pub C, pub Object<'js>);

impl<'js, C> IntoJs<'js> for WithProto<'js, C>
where
    C: ClassDef,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Class::<C>::instance_proto(ctx, self.0, self.1).map(|Class(obj, _)| Value::Object(obj))
    }
}

/// The macro to simplify class definition.
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
#[macro_export]
macro_rules! class_def {
    ($name:ident $($rest:tt)*) => {
        $crate::class_def!{@decl $name
                           $crate::class_def!{@parse $($rest)*}}
    };

    (@parse ($proto:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@proto _ctx $proto $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse ($ctx:ident, $proto:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@proto $ctx $proto $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse ~($self:ident, $marker:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@mark $self $marker $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse) => {};

    (@proto $ctx:ident $proto:ident $($body:tt)*) => {
        const HAS_PROTO: bool = true;
        fn init_proto<'js>($ctx: $crate::Ctx<'js>, $proto: &$crate::Object<'js>) -> $crate::Result<()> {
            $($body)*
            Ok(())
        }
    };

    (@mark $self:ident $marker:ident $($body:tt)*) => {
        const HAS_REFS: bool = true;
        fn mark_refs(&self, $marker: &$crate::RefsMarker) {
            let $self = self;
            $($body)*
        }
    };

    (@decl $name:ident $($body:tt)*) => {
        impl $crate::ClassDef for $name {
            const CLASS_NAME: &'static str = stringify!($name);

            fn class_id() -> &'static mut $crate::ClassId {
                static mut CLASS_ID: $crate::ClassId = $crate::ClassId::new();
                unsafe { &mut CLASS_ID }
            }

            $($body)*
        }
    };
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn class_basics() {
        struct Foo(pub StdString);
        class_def!(Foo);

        struct Bar(pub i32);
        class_def!(Bar);

        test_with(|ctx| {
            let global = ctx.globals();

            Class::<Foo>::register(ctx).unwrap();
            Class::<Bar>::register(ctx).unwrap();

            global.set("foo", Foo("I'm foo".into())).unwrap();
            global.set("bar", Bar(14)).unwrap();

            let foo: &Foo = global.get("foo").unwrap();
            assert_eq!(foo.0, "I'm foo");

            let bar: &Bar = global.get("bar").unwrap();
            assert_eq!(bar.0, 14);

            if let Err(Error::FromJs { from, to, .. }) = global.get::<_, &Bar>("foo") {
                assert_eq!(from, "object");
                assert_eq!(to, "Bar");
            } else {
                panic!("An error was expected");
            }

            if let Err(Error::FromJs { from, to, .. }) = global.get::<_, &Foo>("bar") {
                assert_eq!(from, "object");
                assert_eq!(to, "Foo");
            } else {
                panic!("An error was expected");
            }

            // which doesn't fail
            Class::<Bar>::register(ctx).unwrap();
            Class::<Foo>::register(ctx).unwrap();
        });

        test_with(|ctx| {
            // which doesn't fail too
            Class::<Foo>::register(ctx).unwrap();
            Class::<Bar>::register(ctx).unwrap();
        });
    }

    #[test]
    fn point_class() {
        struct Point {
            pub x: f64,
            pub y: f64,
        }

        impl Point {
            pub fn new(x: f64, y: f64) -> Self {
                Self { x, y }
            }

            pub fn zero() -> Self {
                Self::new(0.0, 0.0)
            }

            pub fn get_x(&self) -> f64 {
                self.x
            }
        }

        class_def!(
            Point (proto) {
                proto.set("get_x", JsFn::new("get_x", Method(Point::get_x)))?;
                proto.set("get_y", JsFn::new("get_y", Method(|Point { y, .. }: &Point| *y)))?;
            }
        );

        test_with(|ctx| {
            Class::<Point>::register(ctx).unwrap();

            let global = ctx.globals();

            let ctor = Function::new(ctx, "Point", Class::<Point>::constructor(Point::new))
                .unwrap()
                .into_object();

            ctor.set("zero", JsFn::new("zero", Point::zero)).unwrap();

            {
                let proto: Object = ctor.get("prototype").unwrap();
                let ctor_: Function = proto.get("constructor").unwrap();
                assert_eq!(ctor_.into_object(), ctor);
            }

            global.set("Point", ctor).unwrap();

            let res: f64 = ctx
                .eval(
                    r#"
                        let p = new Point(2, 3);
                        let z = Point.zero();
                        (p.get_x() + z.get_x()) * (p.get_y() + z.get_y())
                    "#,
                )
                .unwrap();
            assert_eq!(res, 6.0);

            let res: f64 = ctx
                .eval(
                    r#"
                        class ColorPoint extends Point {
                            constructor(x, y, color) {
                                super(x, y);
                                this.color = color;
                            }
                            get_color() {
                                return this.color;
                            }
                        }
                        let c = new ColorPoint(3, 5, 2);
                        c.get_x() * c.get_y() + c.get_color()
                    "#,
                )
                .unwrap();
            assert_eq!(res, 17.0);
        });
    }

    mod internal_refs {
        use super::*;
        use std::collections::HashSet;

        struct A {
            name: StdString,
            refs: HashSet<Persistent<Class<'static, A>>>,
        }

        impl Drop for A {
            fn drop(&mut self) {
                println!("A::drop {}", self.name);
            }
        }

        impl A {
            fn new(name: StdString) -> Self {
                println!("A::new {}", name);
                Self {
                    name,
                    refs: HashSet::new(),
                }
            }
        }

        impl<'js> Class<'js, A> {
            pub fn add(mut self, val: Persistent<Class<'static, A>>) {
                self.as_mut().refs.insert(val);
            }

            pub fn rm(mut self, val: Persistent<Class<'static, A>>) {
                self.as_mut().refs.remove(&val);
            }
        }

        class_def!(
            A (proto) {
                println!("A::register");
                proto.set("add", JsFn::new("add", Method(Class::<A>::add)))?;
                proto.set("rm", JsFn::new("rm", Method(Class::<A>::rm)))?;
            }
            ~(this, marker) {
                println!("A::mark {}", this.name);
                for obj in &this.refs {
                    marker.mark(obj);
                }
            }
        );

        #[test]
        fn single_ref() {
            test_with(|ctx| {
                Class::<A>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("A", JsFn::new("A", Class::<A>::constructor(A::new)))
                    .unwrap();

                // a -> b
                let _: () = ctx
                    .eval(
                        r#"
                        let a = new A("a");
                        let b = new A("b");
                        //a.add(b);
                        b.add(a);
                    "#,
                    )
                    .unwrap();
            });
        }

        #[test]
        fn cross_refs() {
            test_with(|ctx| {
                Class::<A>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("A", JsFn::new("A", Class::<A>::constructor(A::new)))
                    .unwrap();

                // a -> b
                // b -> a
                let _: () = ctx
                    .eval(
                        r#"
                        let a = new A("a");
                        let b = new A("b");
                        a.add(b);
                        b.add(a);
                    "#,
                    )
                    .unwrap();
            });
        }

        #[test]
        fn ref_loops() {
            test_with(|ctx| {
                Class::<A>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("A", JsFn::new("A", Class::<A>::constructor(A::new)))
                    .unwrap();

                // a -> b
                // b -> c
                // c -> a
                let _: () = ctx
                    .eval(
                        r#"
                        let a = new A("a");
                        let b = new A("b");
                        let c = new A("c");
                        a.add(b);
                        b.add(c);
                        c.add(a);
                    "#,
                    )
                    .unwrap();
            });
        }

        #[test]
        fn managed_rm() {
            test_with(|ctx| {
                Class::<A>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("A", JsFn::new("A", Class::<A>::constructor(A::new)))
                    .unwrap();

                let _: () = ctx
                    .eval(
                        r#"
                        let a = new A("a");
                        let b = new A("b");
                        a.add(b);
                        b.add(a);
                        a.rm(b);
                        b.rm(a);
                    "#,
                    )
                    .unwrap();
            });
        }
    }
}
