use rquickjs::{Created, Ctx, Func, Loaded, Module, ModuleDef, Native, Result};

pub struct NativeModule;

impl ModuleDef for NativeModule {
    fn load<'js>(_ctx: Ctx<'js>, module: &Module<'js, Created>) -> Result<()> {
        module.add("n")?;
        module.add("s")?;
        module.add("f")?;
        Ok(())
    }

    fn eval<'js>(_ctx: Ctx<'js>, module: &Module<'js, Loaded<Native>>) -> Result<()> {
        module.set("n", 123)?;
        module.set("s", "abc")?;
        module.set("f", Func::new("f", |a: f64, b: f64| (a + b) * 0.5))?;
        Ok(())
    }
}
