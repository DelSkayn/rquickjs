use std::io::{stdout, Write};

use rquickjs::{Context, Function, Object, Result, Runtime, Value};

fn print(s: String) {
    println!("{s}");
}

fn main() -> Result<()> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;

    ctx.with(|ctx| -> Result<()> {
        let global = ctx.globals();
        global.set(
            "print",
            Function::new(ctx.clone(), print)?.with_name("print")?,
        )?;
        ctx.eval::<(), _>(
            r#"
globalThis.console = {
  log(v){
    globalThis.print(`${v}`);
  }
}
"#,
        )?;

        let console: Object = global.get("console")?;
        let js_log: Function = console.get("log")?;
        loop {
            let mut input = String::new();
            let _ = stdout().write(b"> ")?;
            stdout().flush()?;
            std::io::stdin().read_line(&mut input)?;
            let ret = ctx.eval::<Value, _>(input.as_bytes())?;
            js_log.call::<(Value<'_>,), ()>((ret,))?;
        }
    })?;

    Ok(())
}
