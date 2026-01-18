use rquickjs::{CatchResultExt, Class, Coerced, Context, Runtime};

use self::class::MyClass;

mod class;

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let global = ctx.globals();
        Class::<MyClass>::define(&global).unwrap();

        println!("Calling toString");
        let result: String = ctx
            .eval(
                r#"
const a = new MyClass("Hello, world!");
a.toString()
"#,
            )
            .catch(&ctx)
            .unwrap();
        assert_eq!(result, "MyClass(Hello, world!)");

        println!("Calling toJSON");
        let result: String = ctx
            .eval(
                r#"
const b = new MyClass("Hello, world!");
JSON.stringify(b)
"#,
            )
            .catch(&ctx)
            .unwrap();
        assert_eq!(result, "{\"data\":\"Hello, world!\"}");

        println!("Calling toPrimitive");
        let result: Coerced<String> = ctx
            .eval(
                r#"
const c = new MyClass("Hello, world!");
c
"#,
            )
            .catch(&ctx)
            .unwrap();
        assert_eq!(result.0, "Hello, world!");
    });
}
