use rquickjs::{
    AfterInit, BeforeInit, Context, Ctx, Error, FileResolver, Function, Loader, Module, ModuleDef,
    NativeLoader, Resolver, Result, Runtime, ScriptLoader,
};

struct BundleResolver;

impl Resolver for BundleResolver {
    fn resolve(&mut self, _ctx: Ctx, base: &str, name: &str) -> Result<String> {
        if name.starts_with("bundle/") {
            Ok(name.into())
        } else {
            Err(Error::resolving::<_, _, &str>(base, name, None))
        }
    }
}

struct BundleLoader;

impl Loader for BundleLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js, BeforeInit>> {
        if name == "bundle/script_module" {
            ctx.compile_only(
                name,
                r#"
export const n = 123;
export const s = "abc";
export const f = (a, b) => (a + b) * 0.5;
"#,
            )
        } else if name == "bundle/native_module" {
            Module::new::<NativeModule, _>(ctx, name)
        } else {
            Err(Error::loading::<_, &str>(name, None))
        }
    }
}

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

fn main() {
    let resolver = (
        BundleResolver,
        FileResolver::default()
            .add_path("./")
            .add_path("../../target/debug")
            .add_native()
            .build(),
    );
    let loader = (
        BundleLoader,
        ScriptLoader::default(),
        NativeLoader::default(),
    );

    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    rt.set_loader(resolver, loader);
    ctx.with(|ctx| {
        let global = ctx.globals();
        global
            .set(
                "print",
                Function::new2(ctx, "print", |msg: String| println!("{}", msg)).unwrap(),
            )
            .unwrap();

        println!("import script module");
        ctx.compile(
            "test",
            r#"
import { n, s, f } from "script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap();

        println!("import native module");
        ctx.compile(
            "test",
            r#"
import { n, s, f } from "native_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
                "#,
        )
        .unwrap();

        println!("import bundled script module");
        ctx.compile(
            "test",
            r#"
import { n, s, f } from "bundle/script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap();

        println!("import bundled native module");
        ctx.compile(
            "test",
            r#"
import { n, s, f } from "bundle/native_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap();
    });
}
