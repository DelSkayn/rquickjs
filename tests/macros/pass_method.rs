use rquickjs::{
    atom::PredefinedAtom, class::Trace, prelude::Func, CatchResultExt, Class, Context, Ctx, Object,
    Result, Runtime,
};

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct TestClass {
    value: u32,
    another_value: u32,
}

#[rquickjs::methods]
impl TestClass {
    #[qjs(constructor)]
    pub fn new(value: u32) -> Self {
        TestClass {
            value,
            another_value: value,
        }
    }

    #[qjs(get, rename = "value")]
    pub fn get_value(&self) -> u32 {
        self.value
    }

    #[qjs(set, rename = "value")]
    pub fn set_value(&mut self, v: u32) {
        self.value = v
    }

    #[qjs(get, rename = "anotherValue", enumerable)]
    pub fn get_another_value(&self) -> u32 {
        self.another_value
    }

    #[qjs(set, rename = "anotherValue", enumerable)]
    pub fn set_another_value(&mut self, v: u32) {
        self.another_value = v
    }

    #[qjs(static)]
    pub fn compare(a: &Self, b: &Self) -> bool {
        a.value == b.value && a.another_value == b.another_value
    }

    #[qjs(skip)]
    pub fn inner_function(&self) {}

    #[qjs(rename = PredefinedAtom::SymbolIterator)]
    pub fn iterate<'js>(&self, ctx: Ctx<'js>) -> Result<Object<'js>> {
        let res = Object::new(ctx)?;

        res.set(
            PredefinedAtom::Next,
            Func::from(|ctx: Ctx<'js>| -> Result<Object<'js>> {
                let res = Object::new(ctx)?;
                res.set(PredefinedAtom::Done, true)?;
                Ok(res)
            }),
        )?;
        Ok(res)
    }
}

pub fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        Class::<TestClass>::define(&ctx.globals()).unwrap();
        ctx.globals()
            .set(
                "t",
                TestClass {
                    value: 1,
                    another_value: 2,
                },
            )
            .unwrap();

        ctx.eval::<(), _>(
            r#"
            if(t.value !== 1){
                throw new Error(1)
            }
            if(t.anotherValue !== 2){
                throw new Error(2)
            }
            t.value = 5;
            if(t.value !== 5){
                throw new Error(3)
            }
            let nv = new TestClass(5);
            if(nv.value !== 5){
                throw new Error(4)
            }
            t.anotherValue = 5;
            if(!TestClass.compare(t,nv)){
                throw new Error(5)
            }
            if(nv.inner_function !== undefined){
                throw new Error(6)
            }
            let proto = TestClass.prototype;
            if(!Object.keys(proto).includes("anotherValue")){
                throw new Error(7)
            }
            if(Object.keys(proto).includes("value")){
                throw new Error(8)
            }
            for(const v of t){
                throw new Error("iterator should be done immediately")
            }
        "#,
        )
        .catch(&ctx)
        .unwrap();
    });
}
