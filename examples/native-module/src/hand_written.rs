use rquickjs::{module::ModuleDef, Function};

pub struct NativeModule;

impl ModuleDef for NativeModule {
    fn declare(declare: &mut rquickjs::module::Declarations) -> rquickjs::Result<()> {
        declare.declare("n")?;
        declare.declare("s")?;
        declare.declare("f")?;
        Ok(())
    }

    fn evaluate<'js>(
        ctx: rquickjs::Ctx<'js>,
        exports: &mut rquickjs::module::Exports<'js>,
    ) -> rquickjs::Result<()> {
        exports.export("n", 123)?;
        exports.export("s", "abc")?;
        exports.export(
            "f",
            Function::new(ctx, |a: f64, b: f64| (a + b) * 0.5)?.with_name("f"),
        )?;
        Ok(())
    }
}
