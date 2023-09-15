use rquickjs::{module::ModuleDef, Ctx, Function, Result};

pub struct NativeModule;

impl ModuleDef for NativeModule {
    fn declare(decl: &mut rquickjs::module::Declarations) -> Result<()> {
        decl.declare("n")?;
        decl.declare("s")?;
        decl.declare("f")?;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &mut rquickjs::module::Exports<'js>) -> Result<()> {
        exports.export("n", 123)?;
        exports.export("s", "abc")?;
        exports.export(
            "f",
            Function::new(ctx.clone(), |a: f64, b: f64| (a + b) * 0.5)?.with_name("f")?,
        )?;
        Ok(())
    }
}
