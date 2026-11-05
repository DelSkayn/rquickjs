use core::sync::atomic::AtomicI32;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{
    class::{ClassKind, JsClass, Readable, Trace, Tracer, Writable},
    function::This,
    test_with,
    value::Constructor,
    CatchResultExt, Class, Context, Ctx, FromIteratorJs, FromJs, Function, IntoJs, JsLifetime,
    Object, Result, Runtime, Value,
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

    fn constructor(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::value::Constructor<'js>>> {
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
fn instance_of_non_class_object_does_not_leave_pending_exception() {
    test_with(|ctx| {
        // A plain object isn't backed by any Rust class, so the underlying
        // class-id check inside `instance_of` genuinely fails here (as
        // opposed to checking one Rust class against another, where the
        // class ids match and only a later, exception-free v-table
        // comparison tells the classes apart).
        let plain = Object::new(ctx.clone()).unwrap();

        assert!(!plain.instance_of::<Vec3>());

        // A negative `instance_of` result is an ordinary outcome, not an
        // error: it must not leave a pending exception on the runtime for
        // unrelated code to trip over later.
        assert!(!ctx.has_exception());

        // The context must still be perfectly usable afterwards.
        let sum: i32 = ctx.eval("1 + 1").unwrap();
        assert_eq!(sum, 2);
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
                        Ok(format!("{:?}", this.0.borrow().d))
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

#[test]
fn exotic() {
    pub struct ExoticIterator {
        curr_state: Arc<AtomicI32>,
    }

    impl<'js> Trace<'js> for ExoticIterator {
        fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
    }

    unsafe impl<'js> JsLifetime<'js> for ExoticIterator {
        type Changed<'to> = ExoticIterator;
    }

    impl<'js> JsClass<'js> for ExoticIterator {
        const NAME: &'static str = "ExoticIterator";

        type Mutable = Readable;

        const KIND: ClassKind = ClassKind::Exotic;

        fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::Object<'js>>> {
            Ok(Some(crate::Object::new(ctx.clone())?))
        }

        fn constructor(
            _ctx: &crate::Ctx<'js>,
        ) -> crate::Result<Option<crate::value::Constructor<'js>>> {
            Ok(None)
        }

        fn exotic_get_property(
            this: &crate::class::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
            _receiver: crate::Value<'js>,
        ) -> crate::Result<crate::Value<'js>> {
            println!("Get property [iter]: {}", atom.to_string()?);
            if atom.to_string()? == "next" {
                let state = this.borrow().curr_state.clone();
                Ok(Function::new(ctx.clone(), move |ctx: crate::Ctx<'js>| {
                    // A really awful iterator thats implemented as a handwritten state machine
                    //
                    // Do not use this in production
                    if state.load(Ordering::SeqCst) <= 1 {
                        state.store(2, Ordering::SeqCst);

                        let val = crate::Object::from_iter_js(
                            &ctx,
                            [
                                ("done", false.into_js(&ctx)?),
                                ("value", vec!["hello", "1292"].into_js(&ctx)?),
                            ],
                        )?
                        .into_value();

                        Ok::<crate::Value<'_>, crate::Error>(val)
                    } else if state.load(Ordering::SeqCst) == 2 {
                        state.fetch_add(1, Ordering::SeqCst);

                        let val = crate::Object::from_iter_js(
                            &ctx,
                            [
                                ("done", false.into_js(&ctx)?),
                                (
                                    "value",
                                    vec!["i".into_js(&ctx)?, 43.into_js(&ctx)?].into_js(&ctx)?,
                                ),
                            ],
                        )?
                        .into_value();

                        Ok(val)
                    } else {
                        state.fetch_add(1, Ordering::SeqCst);

                        let val = crate::Object::from_iter_js(
                            &ctx,
                            [
                                ("done", true.into_js(&ctx)?),
                                ("value", crate::Value::new_undefined(ctx.clone())),
                            ],
                        )?
                        .into_value();

                        Ok(val)
                    }
                })?
                .into_value())
            } else {
                Ok(crate::Value::new_undefined(ctx.clone()))
            }
        }

        fn exotic_has_property(
            this: &super::JsCell<'js, Self>,
            _ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
        ) -> crate::Result<bool> {
            let _ = this;
            Ok(atom.to_string()? == "next")
        }
    }

    #[derive(Clone)]
    pub struct Exotic {
        pub i: i32,
    }

    impl<'js> Trace<'js> for Exotic {
        fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
    }

    unsafe impl<'js> JsLifetime<'js> for Exotic {
        type Changed<'to> = Exotic;
    }

    impl<'js> JsClass<'js> for Exotic {
        const NAME: &'static str = "Exotic";

        type Mutable = Writable;

        const KIND: ClassKind = ClassKind::Exotic;

        fn prototype(ctx: &crate::Ctx<'js>) -> crate::Result<Option<crate::Object<'js>>> {
            Ok(Some(crate::Object::new(ctx.clone())?))
        }

        fn constructor(
            _ctx: &crate::Ctx<'js>,
        ) -> crate::Result<Option<crate::value::Constructor<'js>>> {
            Ok(None)
        }

        fn exotic_get_property(
            this: &crate::class::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
            _receiver: crate::Value<'js>,
        ) -> crate::Result<crate::Value<'js>> {
            let symbol_iterator = crate::Atom::from_predefined(
                ctx.clone(),
                crate::atom::PredefinedAtom::SymbolIterator,
            );
            println!("Get property: {}", atom.to_string()?);
            if atom.to_string()? == "hello" {
                assert!(this.borrow().i == 42);
                Ok("world".into_js(ctx)?)
            } else if atom.to_string()? == "toString" {
                Ok(Function::new(ctx.clone(), || {
                    let f = "class Exotic { [native code] }";
                    Ok::<&'static str, crate::Error>(f)
                })?
                .into_value())
            } else if atom == symbol_iterator {
                println!("Getting iterator");
                let exotic = Class::<ExoticIterator>::instance(
                    ctx.clone(),
                    ExoticIterator {
                        curr_state: Arc::default(),
                    },
                )?;
                println!("Returning ExoticIterator");
                Ok(Function::new(ctx.clone(), move || {
                    Ok::<crate::Value<'_>, crate::Error>(exotic.clone().into_value())
                })?
                .into_value())
            } else {
                Ok(crate::Value::new_null(ctx.clone()))
            }
        }

        fn exotic_set_property(
            this: &super::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
            _receiver: crate::Value<'js>,
            _value: crate::Value<'js>,
        ) -> crate::Result<bool> {
            let _ = this;
            if atom.to_string()? == "i" {
                let Some(new_i) = _value.as_int() else {
                    let err_val =
                        crate::String::from_str(ctx.clone(), "i must be an integer")?.into_value();
                    return Err(ctx.throw(err_val));
                };
                this.borrow_mut().i = new_i;
                return Ok(true);
            }
            let err_val =
                crate::String::from_str(ctx.clone(), "Properties are read-only")?.into_value();
            Err(ctx.throw(err_val))
        }

        fn exotic_has_property(
            this: &super::JsCell<'js, Self>,
            _ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
        ) -> crate::Result<bool> {
            let _ = this;
            println!("Got atom: {}", atom.to_string()?);
            Ok(atom.to_string()? == "hello"
                || atom.to_string()? == "i"
                || atom.to_string()? == "toString")
        }

        fn exotic_delete_property(
            _this: &super::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
            _atom: crate::Atom<'js>,
        ) -> crate::Result<bool> {
            let err_val =
                crate::String::from_str(ctx.clone(), "Properties cannot be deleted")?.into_value();
            Err(ctx.throw(err_val))
        }

        fn exotic_get_own_property(
            this: &super::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
            atom: crate::Atom<'js>,
        ) -> crate::Result<Option<super::PropertyDescriptor<'js>>> {
            let name = atom.to_string()?;
            if name == "hello" || name == "i" {
                let value = if name == "hello" {
                    "world".into_js(ctx)?
                } else {
                    this.borrow().i.into_js(ctx)?
                };
                Ok(Some(super::PropertyDescriptor::new_value(
                    value, true, true, false,
                )))
            } else {
                Ok(None)
            }
        }

        fn exotic_get_own_property_names(
            _this: &super::JsCell<'js, Self>,
            ctx: &crate::Ctx<'js>,
        ) -> crate::Result<Vec<super::PropertyName<'js>>> {
            Ok(vec![
                super::PropertyName {
                    atom: crate::Atom::from_str(ctx.clone(), "hello")?,
                    is_enumerable: true,
                },
                super::PropertyName {
                    atom: crate::Atom::from_str(ctx.clone(), "i")?,
                    is_enumerable: true,
                },
            ])
        }
    }

    test_with(|ctx| {
        let exotic = Class::<Exotic>::instance(ctx.clone(), Exotic { i: 0 }).unwrap();
        ctx.globals().set("exotic", exotic).unwrap();
        ctx.globals()
            .set(
                "assert",
                Function::new(
                    ctx.clone(),
                    |ctx: crate::Ctx<'_>, cond: bool, msg: String| {
                        if !cond {
                            let err_val = crate::String::from_str(ctx.clone(), &msg)?.into_value();
                            return Err(ctx.throw(err_val));
                        }
                        Ok(())
                    },
                ),
            )
            .unwrap();

        let v = ctx
                .eval::<String, _>(
                    r"
                if(exotic.foo !== null) {
                    throw new Error('foo should be null');
                }
                try {
                    exotic.foo = {}
                } catch(e) {
                    if (e?.toString() !== 'Properties are read-only') {
                        throw new Error('wrong error message: ' + e?.toString());
                    }
                }
                if (exotic.foo !== null) {
                    throw new Error('foo should be null');
                }
                exotic.i = 42;
                if (exotic.hello === 42) {
                    throw new Error('i should be 42');
                }
                assert(exotic?.toString() === 'class Exotic { [native code] }', `exotic.toString() should be 'class Exotic { [native code] }' but is ${exotic?.toString()}`);
                assert('i' in exotic, 'i should be in exotic');
                assert('hello' in exotic, 'hello should be in exotic');
                assert(!('foo' in exotic), 'foo should not be in exotic');

                try {
                    delete exotic.i;
                } catch(e) {
                    if (e?.toString() !== 'Properties cannot be deleted') {
                        throw new Error('wrong error message: ' + e?.toString());
                    }
                }

                let resp = []
                for (let [objKey, value] of exotic) {
                    if (objKey !== 'i' && objKey !== 'hello') {
                        throw new Error('only i and hello should be enumerable, got ' + objKey);
                    }
                    resp.push(`${objKey}:${value}`);
                }

                assert(resp.toString() === 'hello:1292,i:43', `${resp.toString()} with length ${resp.length} should be [] as properties are not enumerable`);

                // Test Object.getOwnPropertyNames() (uses get_own_property_names)
                let ownNames = Object.getOwnPropertyNames(exotic);
                assert(ownNames.length === 2, `getOwnPropertyNames should return 2, got ${ownNames.length}`);
                assert(ownNames.includes('hello'), 'getOwnPropertyNames should include hello');
                assert(ownNames.includes('i'), 'getOwnPropertyNames should include i');

                // Test Object.keys() (uses get_own_property_names + get_own_property)
                let keys = Object.keys(exotic);
                assert(keys.length === 2, `Object.keys should return 2 keys, got ${keys.length}`);

                // Test Object.getOwnPropertyDescriptor() (uses get_own_property)
                let desc = Object.getOwnPropertyDescriptor(exotic, 'hello');
                assert(desc !== undefined, 'descriptor for hello should exist');
                assert(desc.value === 'world', `descriptor value should be world, got ${desc.value}`);
                assert(desc.configurable === true, 'hello should be configurable');
                assert(desc.enumerable === true, 'hello should be enumerable');
                assert(desc.writable === false, 'hello should not be writable');

                // Non-existent property returns undefined descriptor
                assert(Object.getOwnPropertyDescriptor(exotic, 'nonexistent') === undefined, 'nonexistent should be undefined');

                exotic.hello
            ",
                )
                .catch(&ctx)
                .unwrap();

        assert_eq!(v, "world");
    })
}

struct Thing {}
impl<'js> JsClass<'js> for Thing {
    const NAME: &'static str = "Thing";
    type Mutable = Writable;
    fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        let proto = Object::new(ctx.clone())?;
        Ok(Some(proto))
    }
    fn constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        let constr = Constructor::new_class::<Thing, _, _>(ctx.clone(), || Thing {})?;
        Ok(Some(constr))
    }
}
impl<'js> IntoJs<'js> for Thing {
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let cls = Class::<Self>::instance(ctx.clone(), self)?;
        IntoJs::into_js(cls, ctx)
    }
}
impl<'js> Trace<'js> for Thing {
    fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
}
unsafe impl<'js> JsLifetime<'js> for Thing {
    type Changed<'to> = Thing;
}

fn check(ctx: &Context) -> Vec<bool> {
    ctx.with(|c| {
        Class::<Thing>::define(&c.globals()).unwrap();
        let t = Class::instance(c.clone(), Thing {}).unwrap();
        c.globals().set("t", t).unwrap();
        c.globals()
            .set("f", Function::new(c.clone(), || 1).unwrap())
            .unwrap();
        c.eval(
            "[
                    Object.getPrototypeOf(Thing.prototype) === Object.prototype,
                    Object.getPrototypeOf(t) === Thing.prototype,
                    t instanceof Thing,
                    t instanceof Object,
                    Object.getPrototypeOf(f) === Function.prototype,
                    f instanceof Function,
                ]",
        )
        .unwrap()
    })
}

#[test]
fn ensure_correct_class_prototypes() {
    let rt = Runtime::new().unwrap();
    let a = Context::full(&rt).unwrap();
    let b = Context::full(&rt).unwrap();
    assert!(check(&a).iter().all(|x| *x));
    assert!(check(&b).iter().all(|x| *x));
}
