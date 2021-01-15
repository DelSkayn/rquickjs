mod refs;

use crate::{
    handle_exception, qjs, ClassId, Ctx, Error, FromJs, Function, IntoJs, Object, Outlive, Result,
    Type, Value,
};
use std::{ffi::CString, marker::PhantomData, mem, ops::Deref, ptr};

pub use refs::{HasRefs, RefsMarker};

/// The ES6 class definition trait
///
/// This trait helps export rust data types to QuickJS so JS code can operate with it as a ES6 classes.
///
/// Do not need implements this trait manually. Instead you can use [`class_def`](crate::class_def) macros or [`bind`](attr.bind.html) attribute to bind classes with methods in easy way.
///
/// ```
/// # use rquickjs::{ClassId, ClassDef, Ctx, Object, Result, RefsMarker};
/// struct MyClass;
///
/// impl ClassDef for MyClass {
///     const CLASS_NAME: &'static str = "MyClass";
///
///     unsafe fn class_id() -> &'static mut ClassId {
///         static mut CLASS_ID: ClassId = ClassId::new();
///         &mut CLASS_ID
///     }
///
///     // With prototype
///     const HAS_PROTO: bool = true;
///     fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
///         Ok(())
///     }
///
///     // With statics
///     const HAS_STATIC: bool = true;
///     fn init_static<'js>(ctx: Ctx<'js>, ctor: &Object<'js>) -> Result<()> {
///         Ok(())
///     }
///
///     // With internal references
///     const HAS_REFS: bool = true;
///     fn mark_refs(&self, marker: &RefsMarker) {
///         // marker.mark(&self.some_persistent_value);
///     }
/// }
/// ```
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub trait ClassDef {
    /// The name of a class
    const CLASS_NAME: &'static str;

    /// The reference to class identifier
    ///
    /// # Safety
    /// This method should return reference to mutable static class id which should be initialized to zero.
    unsafe fn class_id() -> &'static mut ClassId;

    /// The class has prototype
    const HAS_PROTO: bool = false;

    /// The prototype initializer method
    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }

    /// The class has static data
    const HAS_STATIC: bool = false;

    /// The static initializer method
    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
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

    /// Convert an instance of class into JS object
    ///
    /// This method helps implement [`IntoJs`] trait for classes
    fn into_js_obj<'js>(self, ctx: Ctx<'js>) -> Result<Value<'js>>
    where
        Self: Sized,
    {
        Class::<Self>::instance(ctx, self).map(|Class(Object(val), _)| val)
    }

    /// Get reference from JS object
    ///
    /// This method helps implement [`FromJs`] trait for classes
    fn from_js_ref<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        Class::<Self>::try_ref(ctx, &value)
    }

    /// Get mutable reference from JS object
    ///
    /// This method helps implement [`FromJs`] trait for classes
    fn from_js_mut<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        Class::<Self>::try_mut(ctx, &value)
    }

    /// Get an instance of class from JS object
    fn from_js_obj<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = Class::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}

/// The class object interface
///
// FIXME: Maybe it should be private.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct Class<'js, C>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, 't, C> Outlive<'t> for Class<'js, C> {
    type Target = Class<'t, C>;
}

impl<'js, C> Deref for Class<'js, C> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, C> AsRef<Object<'js>> for Class<'js, C> {
    fn as_ref(&self) -> &Object<'js> {
        &self.0
    }
}

impl<'js, C> AsRef<Value<'js>> for Class<'js, C> {
    fn as_ref(&self) -> &Value<'js> {
        &(self.0).0
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
    /// Get an integer class identifier
    #[inline(always)]
    pub(crate) fn id() -> qjs::JSClassID {
        unsafe { C::class_id() }.get()
    }

    /// Wrap constructor of class
    #[inline(always)]
    pub fn constructor<F>(func: F) -> Constructor<C, F> {
        Constructor(func, PhantomData)
    }

    /// Initialize static data
    pub fn static_init(ctx: Ctx<'js>, func: &Function<'js>) -> Result<()> {
        if C::HAS_STATIC {
            C::init_static(ctx, func.as_object())?;
        }
        Ok(())
    }

    /// Instantiate the object of class
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        let val =
            unsafe { handle_exception(ctx, qjs::JS_NewObjectClass(ctx.ctx, Self::id() as _)) }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
            PhantomData,
        ))
    }

    /// Instantiate the object of class with given prototype
    pub fn instance_proto(ctx: Ctx<'js>, value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        let val = unsafe {
            handle_exception(
                ctx,
                qjs::JS_NewObjectProtoClass(ctx.ctx, proto.0.as_js_value(), Self::id()),
            )
        }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
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
        let ptr = qjs::JS_GetOpaque2(ctx, value, Self::id()) as *mut C;
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
        let class_id = unsafe { C::class_id() };
        class_id.init();
        let class_id = Self::id();
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
                call: None,
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
    ///
    /// # Safety
    /// This function must only be called from `js_module_init` function or should be called right after context initialization.
    /// From Rust code you should use [Class::register] instead.
    pub unsafe fn register_raw(ctx: *mut qjs::JSContext) {
        Self::register(Ctx::from_ptr(ctx)).unwrap()
    }

    /// Get the own prototype object of a class
    pub fn prototype(ctx: Ctx<'js>) -> Result<Object<'js>> {
        Ok(Object(unsafe {
            let class_id = Self::id();
            let proto = qjs::JS_GetClassProto(ctx.ctx, class_id);
            let proto = Value::from_js_value(ctx, proto);
            let type_ = proto.type_of();
            if type_ == Type::Object {
                proto
            } else {
                return Err(Error::new_from_js_message(
                    type_.as_str(),
                    "prototype",
                    "Unregistered class",
                ));
            }
        }))
    }

    /// Get class from value
    pub fn from_object(value: Object<'js>) -> Result<Self> {
        if value.instance_of::<C>() {
            Ok(Self(value, PhantomData))
        } else {
            Err(Error::new_from_js("object", C::CLASS_NAME))
        }
    }

    /// Get reference to object
    #[inline]
    pub fn as_object(&self) -> &Object<'js> {
        &self.0
    }

    /// Convert to object
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        self.0
    }

    /// Convert to value
    #[inline]
    pub fn into_value(self) -> Value<'js> {
        self.into_object().0
    }

    unsafe extern "C" fn gc_mark(
        rt: *mut qjs::JSRuntime,
        val: qjs::JSValue,
        mark_func: qjs::JS_MarkFunc,
    ) {
        let ptr = qjs::JS_GetOpaque(val, Self::id()) as *mut C;
        debug_assert!(!ptr.is_null());
        let inst = &mut *ptr;
        let marker = RefsMarker { rt, mark_func };
        inst.mark_refs(&marker);
    }

    unsafe extern "C" fn finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let ptr = qjs::JS_GetOpaque(val, Self::id()) as *mut C;
        debug_assert!(!ptr.is_null());
        let inst = Box::from_raw(ptr);
        qjs::JS_FreeValueRT(rt, val);
        mem::drop(inst);
    }
}

impl<'js> Object<'js> {
    /// Check the object for instance of
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
    pub fn instance_of<C: ClassDef>(&self) -> bool {
        let ptr = unsafe { qjs::JS_GetOpaque2(self.0.ctx.ctx, self.0.value, Class::<C>::id()) };
        !ptr.is_null()
    }

    /// Convert object into instance of class
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
    pub fn into_instance<C: ClassDef>(self) -> Option<Class<'js, C>> {
        if self.instance_of::<C>() {
            Some(Class(self, PhantomData))
        } else {
            None
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
        Class::<C>::from_object(value)
    }
}

/// The wrapper for constructor function
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
#[repr(transparent)]
pub struct Constructor<C, F>(pub(crate) F, PhantomData<C>);

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
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct WithProto<'js, C>(pub C, pub Object<'js>);

impl<'js, C> IntoJs<'js> for WithProto<'js, C>
where
    C: ClassDef + IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Class::<C>::instance_proto(ctx, self.0, self.1).map(|Class(Object(val), _)| val)
    }
}

/// The macro to simplify class definition.
///
/// ```
/// # use rquickjs::{class_def, Method, Func};
/// #
/// struct TestClass;
///
/// impl TestClass {
///     fn method(&self) {}
///     fn static_func() {}
/// }
///
/// class_def! {
///     TestClass
///     // optional prototype initializer
///     (proto) {
///         proto.set("method", Func::from(Method(TestClass::method)))?;
///     }
///     // optional static initializer
///     @(ctor) {
///         ctor.set("static_func", Func::from(TestClass::static_func))?;
///     }
///     // optional internal refs marker (for gc)
///     ~(_self, _marker) {
///         // mark internal refs if exists
///     }
/// }
/// ```
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
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

    (@parse @($ctor:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@ctor _ctx $ctor $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse @($ctx:ident, $ctor:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@ctor $ctx $ctor $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse ~($self:ident, $marker:ident) { $($body:tt)* } $($rest:tt)*) => {
        $crate::class_def!{@mark $self $marker $($body)*}
        $crate::class_def!{@parse $($rest)*}
    };

    (@parse ~ $($rest:tt)*) => {
        $crate::class_def!{@mark this marker $crate::HasRefs::mark_refs(this, marker);}
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

    (@ctor $ctx:ident $ctor:ident $($body:tt)*) => {
        const HAS_STATIC: bool = true;
        fn init_static<'js>($ctx: $crate::Ctx<'js>, $ctor: &$crate::Object<'js>) -> $crate::Result<()> {
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

            unsafe fn class_id() -> &'static mut $crate::ClassId {
                static mut CLASS_ID: $crate::ClassId = $crate::ClassId::new();
                &mut CLASS_ID
            }

            $($body)*
        }

        impl<'js> $crate::IntoJs<'js> for $name {
            fn into_js(self, ctx: $crate::Ctx<'js>) -> $crate::Result<$crate::Value<'js>> {
                <$name as $crate::ClassDef>::into_js_obj(self, ctx)
            }
        }

        impl<'js> $crate::FromJs<'js> for &'js $name {
            fn from_js(ctx: $crate::Ctx<'js>, value: $crate::Value<'js>) -> $crate::Result<Self> {
                <$name as $crate::ClassDef>::from_js_ref(ctx, value)
            }
        }

        impl<'js> $crate::FromJs<'js> for &'js mut $name {
            fn from_js(ctx: $crate::Ctx<'js>, value: $crate::Value<'js>) -> $crate::Result<Self> {
                <$name as $crate::ClassDef>::from_js_mut(ctx, value)
            }
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

        class_def! {
            Point (proto) {
                proto.set("get_x", Func::from(Method(Point::get_x)))?;
                proto.set("get_y", Func::from(Method(|Point { y, .. }: &Point| *y)))?;
            } @(ctor) {
                ctor.set("zero", Func::from(Point::zero))?;
            }
        }

        test_with(|ctx| {
            Class::<Point>::register(ctx).unwrap();

            let global = ctx.globals();

            let ctor = Function::new(ctx, Class::<Point>::constructor(Point::new)).unwrap();

            {
                let ctor = ctor.as_object();
                let proto: Object = ctor.get("prototype").unwrap();
                let ctor_: Function = proto.get("constructor").unwrap();
                assert_eq!(&ctor_.into_object(), ctor);
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

    #[test]
    fn concurrent_register() {
        struct X;

        class_def!(
            X (_proto) {
                println!("X::register");
            }
        );

        fn run() {
            test_with(|ctx| {
                Class::<X>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("X", Func::from(Class::<X>::constructor(|| X)))
                    .unwrap();
            });
        }

        let h1 = std::thread::spawn(run);
        let h2 = std::thread::spawn(run);
        let h3 = std::thread::spawn(run);
        let h4 = std::thread::spawn(run);
        let h5 = std::thread::spawn(run);

        h1.join().unwrap();
        h2.join().unwrap();
        h3.join().unwrap();
        h4.join().unwrap();
        h5.join().unwrap();
    }

    mod internal_refs {
        use super::*;
        use std::collections::HashSet;

        struct A {
            name: StdString,
            refs: HashSet<Persistent<Class<'static, A>>>,
        }

        impl HasRefs for A {
            fn mark_refs(&self, marker: &RefsMarker) {
                println!("A::mark {}", self.name);
                self.refs.mark_refs(marker);
            }
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
            A~ (proto) {
                println!("A::register");
                proto.set("add", Func::from(Method(Class::<A>::add)))?;
                proto.set("rm", Func::from(Method(Class::<A>::rm)))?;
            }
        );

        #[test]
        fn single_ref() {
            test_with(|ctx| {
                Class::<A>::register(ctx).unwrap();

                let global = ctx.globals();
                global
                    .set("A", Func::from(Class::<A>::constructor(A::new)))
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
                    .set("A", Func::from(Class::<A>::constructor(A::new)))
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
                    .set("A", Func::from(Class::<A>::constructor(A::new)))
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
                    .set("A", Func::from(Class::<A>::constructor(A::new)))
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
