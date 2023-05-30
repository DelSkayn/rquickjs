use rquickjs::{function::Func, module::ModuleDef, Ctx, Module, Result};

pub struct NativeModule;

impl ModuleDef for NativeModule {
    fn declare<'js>(_ctx: Ctx<'js>, exports: &mut Declarations) -> Result<()> {
        module.declare("n")?;
        module.declare("s")?;
        module.declare("f")?;
        Ok(())
    }

    fn eval<'js>(_ctx: Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        exports.export("n", 123)?;
        exports.export("s", "abc")?;
        exports.export("f", Func::new("f", |a: f64, b: f64| (a + b) * 0.5))?;
        Ok(())
    }
}
