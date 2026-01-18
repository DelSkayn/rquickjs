use rquickjs::{
    class::{inherits::HasParent, Trace},
    CatchResultExt, Class, Context, JsLifetime, Object, Runtime,
};

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename_all = "camelCase")]
pub struct TestClass<'js> {
    #[qjs(get, set)]
    inner_object: Object<'js>,
    #[qjs(get, set)]
    some_value: u32,
    #[qjs(get, set, enumerable)]
    another_value: u32,
}

#[rquickjs::methods]
impl<'js> TestClass<'js> {
    #[qjs(constructor)]
    pub fn new(inner_object: Object<'js>, some_value: u32, another_value: u32) -> Self {
        Self {
            inner_object,
            some_value,
            another_value,
        }
    }

    #[qjs(static)]
    pub fn compare(a: &Self, b: &Self) -> bool {
        a.some_value == b.some_value && a.another_value == b.another_value
    }

    #[qjs(static, rename = "getInnerObject")]
    pub fn get_inner_object(this: &Self) -> Object<'js> {
        this.inner_object.clone()
    }
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class(rename_all = "camelCase", extends = TestClass<'js>)]
pub struct TestSubClass<'js> {
    super_: TestClass<'js>,
    #[qjs(get, set)]
    sub_value: u32,
}

#[rquickjs::methods]
impl<'js> TestSubClass<'js> {
    #[qjs(constructor)]
    pub fn new(
        inner_object: Object<'js>,
        some_value: u32,
        another_value: u32,
        sub_value: u32,
    ) -> Self {
        Self {
            super_: TestClass {
                inner_object,
                some_value,
                another_value,
            },
            sub_value,
        }
    }

    #[qjs(static)]
    pub fn compare(a: &Self, b: &Self) -> bool {
        TestClass::compare(&a.super_, &b.super_) && a.sub_value == b.sub_value
    }
}

pub fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let cls = Class::instance(
            ctx.clone(),
            TestClass {
                inner_object: Object::new(ctx.clone()).unwrap(),
                some_value: 1,
                another_value: 2,
            },
        )
        .unwrap();
        let sub_cls = Class::instance(
            ctx.clone(),
            TestSubClass {
                super_: TestClass {
                    inner_object: Object::new(ctx.clone()).unwrap(),
                    some_value: 1,
                    another_value: 2,
                },
                sub_value: 3,
            },
        )
        .unwrap();
        ctx.globals().set("t", cls.clone()).unwrap();
        ctx.globals().set("t2", sub_cls.clone()).unwrap();
        ctx.globals()
            .set("TestClass", Class::<TestClass>::constructor(&ctx).unwrap())
            .unwrap();
        ctx.globals()
            .set(
                "TestSubClass",
                Class::<TestSubClass>::constructor(&ctx).unwrap(),
            )
            .unwrap();
        ctx.eval::<(), _>(
            r#"
            if(t.someValue !== 1){
                throw new Error(1)
            }
            if(t.anotherValue !== 2){
                throw new Error(2)
            }
            t.someValue = 3;
            if(t.someValue !== 3){
                throw new Error(3)
            }
            let proto = Object.getPrototypeOf(t);
            if(!Object.keys(proto).includes("anotherValue")){
                throw new Error(Object.keys(t).join(","))
            }
            if(Object.keys(proto).includes("someValue")){
                throw new Error(5)
            }
            if(!t.innerObject){
                throw new Error(6)
            }
            if(typeof t.innerObject !== "object"){
                throw new Error(7)
            }
            t.innerObject.test = 42;

            if(t2.someValue !== 1){
                throw new Error(8)
            }
            if(t2.anotherValue !== 2){
                throw new Error(9)
            }
            if(t2.subValue !== 3){
                throw new Error(10)
            }
            t2.someValue = 4;
            t2.subValue = 5;
            if(t2.someValue !== 4){
                throw new Error(11)
            }
            if(t2.subValue !== 5){
                throw new Error(12)
            }
            let proto2 = Object.getPrototypeOf(t2);
            if (Object.getPrototypeOf(proto2) !== proto){
                throw new Error(13)
            }
            if(!t2.innerObject){
                throw new Error(14)
            }
            if(typeof t2.innerObject !== "object"){
                throw new Error(15)
            }
            t2.innerObject.test = 43;

            if(!TestClass.compare(t,t)){
                throw new Error(16)
            }
            if(!TestSubClass.compare(t2,t2)){
                throw new Error(17)
            }
            if(!TestClass.getInnerObject(t).test){
                throw new Error(18)
            }
            if(!TestSubClass.getInnerObject(t2).test){
                throw new Error(19)
            }
        "#,
        )
        .catch(&ctx)
        .unwrap();

        let b = cls.borrow();
        assert_eq!(b.some_value, 3);
        assert_eq!(b.another_value, 2);
        assert_eq!(b.inner_object.get::<_, u32>("test").unwrap(), 42);

        let b2 = sub_cls.borrow();
        assert_eq!(b2.as_parent().some_value, 4);
        assert_eq!(b2.as_parent().another_value, 2);
        assert_eq!(b2.sub_value, 5);
        assert_eq!(
            b2.as_parent().inner_object.get::<_, u32>("test").unwrap(),
            43
        );
    });
}
