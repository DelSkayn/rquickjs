use rquickjs::{
    loader::{
        BuiltinLoader, BuiltinResolver, FileResolver, ModuleLoader, NativeLoader, ScriptLoader,
    },
    Context, Function, Module, Runtime,
};

mod bundle;
use bundle::{NativeModule, SCRIPT_MODULE};

fn print(msg: String) {
    println!("{msg}");
}

fn main() {
    let resolver = (
        BuiltinResolver::default()
            .with_module("bundle/script_module")
            .with_module("bundle/native_module"),
        FileResolver::default()
            .with_path("./")
            .with_path("../../target/debug")
            .with_native(),
    );
    let loader = (
        BuiltinLoader::default().with_module("bundle/script_module", SCRIPT_MODULE),
        ModuleLoader::default().with_module("bundle/native_module", NativeModule),
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
                Function::new(ctx.clone(), print)
                    .unwrap()
                    .with_name("print")
                    .unwrap(),
            )
            .unwrap();

        println!("import script module");
        Module::evaluate(
            ctx.clone(),
            "test",
            r#"
import { n, s, f } from "script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();

        println!("import native module");
        Module::evaluate(
            ctx.clone(),
            "test",
            r#"
import { n, s, f } from "native_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
                "#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();

        println!("import bundled script module");
        Module::evaluate(
            ctx.clone(),
            "test",
            r#"
import { n, s, f } from "bundle/script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();

        println!("import bundled native module");
        Module::evaluate(
            ctx.clone(),
            "test",
            r#"
import { n, s, f } from "bundle/native_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();
    });
}
