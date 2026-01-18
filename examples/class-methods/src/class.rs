use rquickjs::atom::PredefinedAtom;
use rquickjs::class::Trace;
use rquickjs::{Ctx, JsLifetime, Null, Object, Result, Value};

#[derive(Clone, Trace, JsLifetime)]
#[rquickjs::class]
pub struct MyClass {
    #[qjs(skip_trace)]
    data: String,
}

#[rquickjs::methods(rename_all = "camelCase")]
impl MyClass {
    #[qjs(constructor)]
    pub fn new(data: String) -> Self {
        Self { data }
    }

    #[qjs(get)]
    fn data(&self) -> String {
        self.data.clone()
    }

    #[qjs(rename = PredefinedAtom::ToJSON)]
    fn to_json(self, ctx: Ctx<'_>) -> Result<Value<'_>> {
        let obj = Object::new(ctx)?;
        obj.set("data", self.data)?;
        Ok(obj.into_value())
    }

    #[allow(clippy::inherent_to_string)]
    #[qjs(rename = PredefinedAtom::ToString)]
    fn to_string(&self) -> String {
        format!("MyClass({})", self.data)
    }

    #[qjs(rename = PredefinedAtom::SymbolToPrimitive)]
    fn to_primitive(self, ctx: Ctx<'_>, hint: String) -> Result<Value<'_>> {
        if hint == "string" {
            return Ok(rquickjs::String::from_str(ctx, &self.data)?.into_value());
        }
        Ok(Null.into_value(ctx))
    }
}
