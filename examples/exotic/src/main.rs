use rquickjs::{class::Trace, Atom, Context, JsLifetime, Result, Runtime, Value};

#[derive(Trace, JsLifetime)]
#[rquickjs::class(exotic)]
pub struct TestClass {
    value: i32,
}

impl TestClass {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
}

#[rquickjs::exotic]
impl TestClass {
    #[qjs(get)]
    pub fn value(&self, atom: Atom<'_>) -> Option<i32> {
        if atom.to_string().unwrap() == "value" {
            Some(self.value)
        } else {
            None
        }
    }

    #[qjs(set)]
    pub fn set_value(&mut self, atom: Atom<'_>, value: Value<'_>) -> bool {
        if atom.to_string().unwrap() == "value" {
            self.value = value.as_int().unwrap();
            true
        } else {
            false
        }
    }

    #[qjs(has)]
    pub fn has_value(&self, atom: Atom<'_>) -> bool {
        atom.to_string().unwrap() == "value"
    }

    #[qjs(delete)]
    pub fn delete_value(&mut self, atom: Atom<'_>) -> bool {
        if atom.to_string().unwrap() == "value" {
            self.value = 0;
            true
        } else {
            false
        }
    }
}

fn main() -> Result<()> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;

    ctx.with(|ctx| -> Result<()> {
        let global = ctx.globals();

        let my_class = TestClass::new(42);
        global.set("my_class", my_class)?;

        let value = ctx.eval::<u32, _>(r#"my_class.value"#)?;
        println!("value: {}", value);

        let value = ctx.eval::<Option<i32>, _>(r#"my_class.other"#)?;
        println!("value: {:?}", value);

        let value = ctx.eval::<i32, _>(r#"my_class.value = 43; my_class.value"#)?;
        println!("value: {}", value);

        let value = ctx.eval::<i32, _>(r#"delete my_class.value; my_class.value"#)?;
        println!("value: {}", value);

        let value = ctx.eval::<bool, _>(r#""value" in my_class"#)?;
        println!("value: {}", value);

        let value = ctx.eval::<bool, _>(r#""other" in my_class"#)?;
        println!("value: {}", value);

        Ok(())
    })?;

    Ok(())
}
