mod expand;
mod util;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::parse_macro_input;

pub(crate) use proc_macro2::TokenStream as Tokens;
pub(crate) use proc_macro_error::abort;

pub(crate) use expand::Expander;

/**
# An attribute to generate bindings easy

This macro allows register Rust functions, modules and objects to use it from JavaScript.

## Examples

### Single function binding

```
use rquickjs::{Runtime, Context, bind};

#[bind]
fn add2(a: f32, b: f32) -> f32 {
    a + b
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    let glob = ctx.globals();
    register_add2(ctx, glob).unwrap();

    let res: f32 = ctx.eval(r#"add2(1, 2)"#).unwrap();
    assert_eq!(res, 3.0);
});
```

### Module with two functions

```
use rquickjs::{Runtime, Context, bind};

#[bind]
mod math {
    pub fn add2(a: f32, b: f32) -> f32 {
        a + b
    }

    pub fn mul2(a: f32, b: f32) -> f32 {
        a * b
    }
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    let glob = ctx.globals();
    register_math(ctx, glob).unwrap();

    let res: f32 = ctx.eval(r#"math.mul2(3, math.add2(1, 2))"#).unwrap();
    assert_eq!(res, 9.0);
});
```

### Async function binding

```
# #[async_std::main]
# async fn main() {
use rquickjs::{Runtime, Context, Promise, bind};

#[bind]
async fn sleep(msecs: u64) {
    async_std::task::sleep(
        std::time::Duration::from_millis(msecs)
    ).await;
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

let promise: Promise<()> = ctx.with(|ctx| {
    let glob = ctx.globals();
    register_sleep(ctx, glob).unwrap();

    ctx.eval(r#"sleep(100)"#).unwrap()
});

rt.spawn_pending_jobs(true);

promise.await;
# }
```

 */
#[proc_macro_error]
#[proc_macro_attribute]
pub fn bind(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item);
    //let attr = parse_macro_input!(attr);

    let expander = Expander::new();
    expander.expand(&item).into()
}
