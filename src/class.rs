use crate::{qjs, value, Ctx, Error, FromJs, IntoJs, JsObjectRef, Object, Result, Value};
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
}

/// The class object interface
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
///
/// FIXME: Maybe it should be private.
pub struct Class<C>(PhantomData<C>);

impl<C> Class<C>
where
    C: ClassDef,
{
    /// Wrap constructor of class
    pub fn constructor<F>(func: F) -> Constructor<C, F> {
        Constructor(Self(PhantomData), func)
    }

    /// Instantiate the object of class
    pub fn instance<'js>(ctx: Ctx<'js>, value: C) -> Result<Object<'js>> {
        let class_id = *C::class_id().as_ref();
        let val = unsafe {
            value::handle_exception(ctx, qjs::JS_NewObjectClass(ctx.ctx, class_id as _))
        }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(unsafe { Object(JsObjectRef::from_js_value(ctx, val)) })
    }

    /// Instantiate the object of class with given prototype
    pub fn instance_proto<'js>(ctx: Ctx<'js>, value: C, proto: Object<'js>) -> Result<Object<'js>> {
        let class_id = *C::class_id().as_ref();
        let val = unsafe {
            value::handle_exception(
                ctx,
                qjs::JS_NewObjectProtoClass(ctx.ctx, proto.0.as_js_value(), class_id as _),
            )
        }?;
        let ptr = Box::into_raw(Box::new(value));
        unsafe { qjs::JS_SetOpaque(val, ptr as _) };
        Ok(unsafe { Object(JsObjectRef::from_js_value(ctx, val)) })
    }

    /// Get reference from object
    pub fn reference<'js, 'r>(ctx: Ctx<'js>, value: &Object<'js>) -> Result<&'r C> {
        Self::reference_mut(ctx, value).map(|r| &*r)
    }

    /// Get mutable reference from object
    pub fn reference_mut<'js, 'r>(ctx: Ctx<'js>, value: &Object<'js>) -> Result<&'r mut C> {
        let class_id = *C::class_id().as_ref();
        let ptr = unsafe { qjs::JS_GetOpaque2(ctx.ctx, value.0.as_js_value(), class_id) as *mut C };
        if ptr.is_null() {
            return Err(Error::FromJs {
                from: "object",
                to: C::CLASS_NAME,
                message: None,
            });
        }
        Ok(unsafe { &mut *ptr })
    }

    /// Register the class
    pub fn register<'js>(ctx: Ctx<'js>) -> Result<()> {
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };
        let class_id = unsafe { qjs::JS_NewClassID(C::class_id().as_mut()) };
        let class_name = CString::new(C::CLASS_NAME)?;
        if 0 == unsafe { qjs::JS_IsRegisteredClass(rt, class_id) } {
            let class_def = qjs::JSClassDef {
                class_name: class_name.as_ptr(),
                finalizer: Some(Self::finalizer),
                gc_mark: None, //Some(Self::gc_mark),
                call: None,    //Some(Self::call),
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
    pub fn prototype<'js>(ctx: Ctx<'js>) -> Result<Object<'js>> {
        let class_id = *C::class_id().as_ref();
        Ok(Object(unsafe {
            JsObjectRef::from_js_value(ctx, qjs::JS_GetClassProto(ctx.ctx, class_id))
        }))
    }

    /*unsafe extern "C" fn gc_mark(
        _rt: *mut qjs::JSRuntime,
        _val: qjs::JSValue,
        _mark_func: qjs::JS_MarkFunc,
    ) {
    }*/

    unsafe extern "C" fn finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let class_id = *C::class_id().as_ref();
        let data = Box::from_raw(qjs::JS_GetOpaque(val, class_id) as *mut C);
        qjs::JS_FreeValueRT(rt, val);
        mem::drop(data);
    }
}

impl<'js, C> IntoJs<'js> for C
where
    C: ClassDef,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Class::<C>::instance(ctx, self).map(Value::Object)
    }
}

impl<'js, C> FromJs<'js> for &C
where
    C: ClassDef,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let value = Object::from_js(ctx, value)?;
        Class::<C>::reference(ctx, &value)
    }
}

impl<'js, C> FromJs<'js> for &mut C
where
    C: ClassDef,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let value = Object::from_js(ctx, value)?;
        Class::<C>::reference_mut(ctx, &value)
    }
}

/// The wrapper for constructor function
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
#[repr(transparent)]
pub struct Constructor<C, F>(pub Class<C>, pub F);

impl<C, F> AsRef<F> for Constructor<C, F> {
    fn as_ref(&self) -> &F {
        &self.1
    }
}

impl<C, F> Deref for Constructor<C, F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.1
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
        Class::<C>::instance_proto(ctx, self.0, self.1).map(Value::Object)
    }
}

/// The macro to simplify class definition.
///
/// # Features
/// This type is only available if the `classes` feature is enabled.
#[macro_export]
macro_rules! class_def {
    ($name:ident) => {
        $crate::class_def!{@decl $name}
    };

    ($name:ident ($proto:ident) { $($body:tt)* }) => {
        $crate::class_def!{@decl $name
                           $crate::class_def!{@proto _ctx $proto $($body)*}}
    };

    ($name:ident ($ctx:ident, $proto:ident) { $($body:tt)* }) => {
        $crate::class_def!{@decl $name
                           $crate::class_def!{@proto $ctx $proto $($body)*}}
    };

    (@proto $ctx:ident $proto:ident $($body:tt)*) => {
        const HAS_PROTO: bool = true;

        fn init_proto<'js>($ctx: $crate::Ctx<'js>, $proto: &$crate::Object<'js>) -> $crate::Result<()> {
            $($body)*
            Ok(())
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
}
