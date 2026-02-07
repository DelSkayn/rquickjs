use rquickjs::{
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
    Context, Ctx, Error, Function, Module, Result, Runtime,
};

struct ExampleResolver;

impl Resolver for ExampleResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, _base: &str, name: &str) -> Result<String> {
        Ok(name.to_string())
    }
}

struct ExampleLoader;

impl Loader for ExampleLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let module_type = if let Some(ref attrs) = attributes {
            attrs.get_type()?
        } else {
            None
        };

        match name {
            "config" => {
                let json_data = r#"{"hello":"world"}"#;

                if module_type.as_deref() == Some("json") {
                    let source = format!("export default {json_data};");
                    Module::declare(ctx.clone(), name, source)
                } else {
                    Err(Error::new_loading_message(name, "requires type: json"))
                }
            }
            "greeting" => Module::declare(
                ctx.clone(),
                name,
                r#"
export function greet(name) {
    return `Hello, ${name}!`;
}
export const DEFAULT_NAME = "World";
"#,
            ),
            _ => Err(Error::new_loading_message(name, "module not found")),
        }
    }
}

fn print(msg: String) {
    println!("{msg}");
}

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    rt.set_loader(ExampleResolver, ExampleLoader);

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

        println!("import json with attributes");
        Module::evaluate(
            ctx.clone(),
            "example1",
            r#"
import config from "config" with { type: "json" };
print(`name = "${config.name}"`);
print(`version = "${config.version}"`);
"#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();

        println!("import regular module");
        Module::evaluate(
            ctx.clone(),
            "example2",
            r#"
import { greet, DEFAULT_NAME } from "greeting";
print(greet(DEFAULT_NAME));
print(greet("Import Attributes"));
"#,
        )
        .unwrap()
        .finish::<()>()
        .unwrap();
    });
}
