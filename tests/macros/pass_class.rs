use rquickjs::{class::Trace, CatchResultExt, Class, Context, Object, Runtime};

#[derive(Trace)]
#[rquickjs::class(rename_all = "camelCase")]
pub struct TestClass<'js> {
    #[qjs(get, set)]
    inner_object: Object<'js>,
    #[qjs(get, set)]
    some_value: u32,
    #[qjs(get, set, enumerable)]
    another_value: u32,
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
        ctx.globals().set("t", cls.clone()).unwrap();
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
        "#,
        )
        .catch(&ctx)
        .unwrap();

        let b = cls.borrow();
        assert_eq!(b.some_value, 3);
        assert_eq!(b.another_value, 2);
        assert_eq!(b.inner_object.get::<_, u32>("test").unwrap(), 42)
    });
}
