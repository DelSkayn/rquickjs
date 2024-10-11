use std::io::Write;

use rquickjs::{CatchResultExt, Context, Function, Object, Result, Runtime, Value};

fn print(s: String) {
    println!("{s}");
}

fn main() -> Result<()> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;

    ctx.with(|ctx| -> Result<()> {
        let global = ctx.globals();
        global.set(
            "__print",
            Function::new(ctx.clone(), print)?.with_name("__print")?,
        )?;
        ctx.eval::<(), _>(
            r#"
globalThis.console = {
  log(...v) {
    globalThis.__print(`${v.join(" ")}`)
  }
}
"#,
        )?;

        let console: Object = global.get("console")?;
        let js_log: Function = console.get("log")?;
        loop {
            let mut input = String::new();
            print!("> ");
            std::io::stdout().flush()?;
            std::io::stdin().read_line(&mut input)?;
            match ctx.eval::<Value, _>(input.as_bytes()).catch(&ctx) {
                Ok(ret) => match js_log.call::<(Value<'_>,), ()>((ret,)) {
                    Err(err) => {
                        println!("{err}")
                    }
                    Ok(_) => {}
                },
                Err(err) => {
                    println!("{err}");
                }
            }
        }
    })?;

    Ok(())
}
