use rquickjs::{BeforeInit, Ctx, JsFn, Module, ModuleDef, Result};

pub struct NativeModule;

impl ModuleDef for NativeModule {
    fn before_init<'js>(_ctx: Ctx<'js>, module: &Module<'js, BeforeInit>) -> Result<()> {
        module.add("n")?;
        module.add("s")?;
        module.add("f")?;
        Ok(())
    }

    fn after_init<'js>(_ctx: Ctx<'js>, module: &Module<'js>) -> Result<()> {
        module.set("n", 123)?;
        module.set("s", "abc")?;
        module.set("f", JsFn::new("f", |a: f64, b: f64| (a + b) * 0.5))?;
        Ok(())
    }
}
