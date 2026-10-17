use rquickjs::{
    atom::PredefinedAtom, class::Trace, prelude::Func, CatchResultExt, Class, Context, Ctx,
    JsLifetime, Object, Result, Runtime,
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

    #[qjs(static, rename = PredefinedAtom::SymbolHasInstance)]
    pub fn has_instance<'js>(_value: rquickjs::Value<'js>) -> bool {
        false
    }

    #[qjs(static, get, rename = "defaultValue")]
    pub fn default_value() -> u32 {
        42
    }

    #[qjs(static, get, rename = "staticPair")]
    pub fn get_static_pair() -> u32 {
        7
    }

    #[qjs(static, set, rename = "staticPair")]
    pub fn set_static_pair(_v: u32) {}

    #[qjs(skip)]
    pub fn inner_function(&self) {}

    #[qjs(prop, rename = PredefinedAtom::SymbolToStringTag, configurable)]
    pub fn to_string_tag() -> &'static str {
        "TestClass"
    }

    #[qjs(prop, rename = "kind", configurable, enumerable, writable)]
    pub fn kind() -> &'static str {
        "test"
    }

    #[qjs(enumerable)]
    pub fn describe(&self) -> u32 {
        self.value
    }

    #[qjs(writable = false, configurable = false)]
    pub fn frozen(&self) -> u32 {
        self.value
    }

    #[qjs(static, enumerable, writable = false)]
    pub fn create() -> u32 {
        0
    }

    #[qjs(set, rename = "sink", configurable)]
    pub fn set_sink(&mut self, v: u32) {
        self.value = v
    }

    #[qjs(get, rename = "pair", enumerable)]
    pub fn get_pair(&self) -> u32 {
        self.value
    }

    #[qjs(set, rename = "pair", configurable, enumerable = true)]
    pub fn set_pair(&mut self, v: u32) {
        self.value = v
    }

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
            if(typeof TestClass[Symbol.hasInstance] !== "function"){
                throw new Error("static Symbol.hasInstance not attached")
            }
            if(TestClass[Symbol.hasInstance]({}) !== false){
                throw new Error("static Symbol.hasInstance wrong return")
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
            // --- data-property (`#[qjs(prop)]`) assertions ---
            // @@toStringTag must be a data descriptor (value present, no get/set).
            let tagDesc = Object.getOwnPropertyDescriptor(proto, Symbol.toStringTag);
            if(!tagDesc || tagDesc.value !== "TestClass"){
                throw new Error(9)
            }
            if(tagDesc.get !== undefined || tagDesc.set !== undefined){
                throw new Error(10)
            }
            if(tagDesc.configurable !== true || tagDesc.enumerable !== false || tagDesc.writable !== false){
                throw new Error(11)
            }
            // The regression case: calling toString on a fake instance must not throw.
            let fake = Object.create(TestClass.prototype);
            if(Object.prototype.toString.call(fake) !== "[object TestClass]"){
                throw new Error(12)
            }
            // Named data property with writable + enumerable + configurable.
            let kindDesc = Object.getOwnPropertyDescriptor(proto, "kind");
            if(!kindDesc || kindDesc.value !== "test"){
                throw new Error(13)
            }
            if(kindDesc.configurable !== true || kindDesc.enumerable !== true || kindDesc.writable !== true){
                throw new Error(14)
            }
            if(TestClass.defaultValue !== 42){
                throw new Error(15)
            }
            let dvDesc = Object.getOwnPropertyDescriptor(TestClass, "defaultValue");
            if(!dvDesc || typeof dvDesc.get !== "function" || dvDesc.set !== undefined){
                throw new Error(16)
            }
            let spDesc = Object.getOwnPropertyDescriptor(TestClass, "staticPair");
            if(!spDesc || typeof spDesc.get !== "function" || typeof spDesc.set !== "function"){
                throw new Error(17)
            }
            if(proto.defaultValue !== undefined){
                throw new Error(18)
            }
            let describeDesc = Object.getOwnPropertyDescriptor(proto, "describe");
            if(describeDesc.enumerable !== true || describeDesc.writable !== true || describeDesc.configurable !== true){
                throw new Error(19)
            }
            let iterDesc = Object.getOwnPropertyDescriptor(proto, Symbol.iterator);
            if(iterDesc.enumerable !== false || iterDesc.writable !== true || iterDesc.configurable !== true){
                throw new Error(20)
            }
            let frozenDesc = Object.getOwnPropertyDescriptor(proto, "frozen");
            if(frozenDesc.enumerable !== false || frozenDesc.writable !== false || frozenDesc.configurable !== false){
                throw new Error(21)
            }
            let createDesc = Object.getOwnPropertyDescriptor(TestClass, "create");
            if(createDesc.enumerable !== true || createDesc.writable !== false || createDesc.configurable !== true){
                throw new Error(22)
            }
            let sinkDesc = Object.getOwnPropertyDescriptor(proto, "sink");
            if(sinkDesc.get !== undefined || typeof sinkDesc.set !== "function" || sinkDesc.configurable !== true || sinkDesc.enumerable !== false){
                throw new Error(23)
            }
            let sinkTarget = new TestClass(1);
            sinkTarget.sink = 5;
            if(sinkTarget.value !== 5){
                throw new Error(24)
            }
            let pairDesc = Object.getOwnPropertyDescriptor(proto, "pair");
            if(pairDesc.configurable !== true || pairDesc.enumerable !== true){
                throw new Error(25)
            }
        "#,
        )
        .catch(&ctx)
        .unwrap();
    });
}
