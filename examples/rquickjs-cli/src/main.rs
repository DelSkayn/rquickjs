use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use rquickjs::{CatchResultExt, Context, Function, Object, Result, Runtime, Value};

fn print(s: String) {
    println!("{s}");
}

fn main() -> Result<()> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;
    let should_exit = std::sync::Arc::new(AtomicBool::new(false));

    ctx.with(|ctx| -> Result<()> {
        let global = ctx.globals();
        let should_exit_clone = should_exit.clone();

        global.set(
            "__print",
            Function::new(ctx.clone(), print)?.with_name("__print")?,
        )?;

        global.set(
            "exit",
            Function::new(ctx.clone(), move || {
                should_exit_clone.store(true, Ordering::Relaxed);
            })?,
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
            if should_exit.load(Ordering::Relaxed) {
                break;
            }
            let mut input = String::new();
            print!("> ");
            std::io::stdout().flush()?;
            std::io::stdin().read_line(&mut input)?;
            ctx.eval::<Value, _>(input.as_bytes())
                .and_then(|ret| js_log.call::<(Value<'_>,), ()>((ret,)))
                .catch(&ctx)
                .unwrap_or_else(|err| println!("{err}"));
        }
        Ok(())
    })?;

    Ok(())
}
