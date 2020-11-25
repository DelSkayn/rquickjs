use rquickjs::{module_init, AfterInit, BeforeInit, Ctx, Function, Module, ModuleDef, Result};

struct NativeModule;

impl ModuleDef for NativeModule {
    fn before_init<'js>(_ctx: Ctx<'js>, module: &Module<'js, BeforeInit>) -> Result<()> {
        module.add("n")?;
        module.add("s")?;
        module.add("f")?;
        Ok(())
    }

    fn after_init<'js>(ctx: Ctx<'js>, module: &Module<'js, AfterInit>) -> Result<()> {
        module.set("n", 123)?;
        module.set("s", "abc")?;
        module.set(
            "f",
            Function::new2(ctx, "f", |a: f64, b: f64| (a + b) * 0.5)?,
        )?;
        Ok(())
    }
}

module_init!(NativeModule);
