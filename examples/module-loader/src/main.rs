use rquickjs::{
    BuiltinLoader, BuiltinResolver, Context, FileResolver, Func, ModuleLoader, NativeLoader,
    Runtime, ScriptLoader,
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
        global.set("print", Func::new("print", print)).unwrap();

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
