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
use rquickjs::{Runtime, Context, ObjectDef, bind};

#[bind]
pub fn add2(a: f32, b: f32) -> f32 {
    a + b
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    let glob = ctx.globals();
    Add2::init(ctx, &glob).unwrap();

    let res: f32 = ctx.eval(r#"add2(1, 2)"#).unwrap();
    assert_eq!(res, 3.0);
});
```

### Module with two functions

```
use rquickjs::{Runtime, Context, Object, bind};

#[bind]
pub mod math {
    pub const PI: f32 = core::f32::consts::PI;

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
    glob.set("math", Object::new_def::<Math>(ctx).unwrap()).unwrap();

    let res: f32 = ctx.eval(r#"math.mul2(3, math.add2(1, 2))"#).unwrap();
    assert_eq!(res, 9.0);
});
```

### Async function binding

```
# #[async_std::main]
# async fn main() {
use rquickjs::{Runtime, Context, Promise, ObjectDef, bind};

#[bind]
pub async fn sleep(msecs: u64) {
    async_std::task::sleep(
        std::time::Duration::from_millis(msecs)
    ).await;
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

rt.spawn_pending_jobs(None);

let promise: Promise<()> = ctx.with(|ctx| {
    let glob = ctx.globals();
    Sleep::init(ctx, &glob).unwrap();

    ctx.eval(r#"sleep(100)"#).unwrap()
});

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
