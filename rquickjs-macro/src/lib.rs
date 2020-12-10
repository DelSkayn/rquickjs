mod bind;
mod config;
mod context;
mod derive;
mod shim;

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::parse_macro_input;

use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, emit_error as error, emit_warning as warning};

use bind::*;
use config::*;
use context::*;
use derive::*;
use shim::*;

/**
# An attribute to generate bindings easy

This macro allows register Rust functions, modules and objects to use it from JavaScript.

## Examples

### Single function binding

```
use rquickjs::{Runtime, Context, bind};

#[bind(object)]
pub fn add2(a: f32, b: f32) -> f32 {
    a + b
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    let glob = ctx.globals();
    glob.init_def::<Add2>().unwrap();

    let res: f32 = ctx.eval(r#"add2(1, 2)"#).unwrap();
    assert_eq!(res, 3.0);
});
```

### Module with two functions

```
use rquickjs::{Runtime, Context, Object, bind};

#[bind(object)]
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
    glob.init_def::<Math>().unwrap();

    let res: f32 = ctx.eval(r#"math.mul2(3, math.add2(1, 2))"#).unwrap();
    assert_eq!(res, 9.0);
});
```

### Bare module definition

```
use rquickjs::{Runtime, Context, Object, bind};

#[bind(object)]
#[quickjs(bare)]
pub mod math {
    #[quickjs(name = "pi")]
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
    glob.init_def::<Math>().unwrap();

    let res: f32 = ctx.eval(r#"mul2(3, add2(1, 2))"#).unwrap();
    assert_eq!(res, 9.0);
});
```

### Async function binding

```
# #[async_std::main]
# async fn main() {
use rquickjs::{Runtime, Context, Promise, bind};

#[bind(object)]
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
    glob.init_def::<Sleep>().unwrap();

    ctx.eval(r#"sleep(100)"#).unwrap()
});

promise.await;
# }
```

 */
#[proc_macro_error]
#[proc_macro_attribute]
pub fn bind(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let attr: AttributeArgs = parse_macro_input!(attr);
    let item = parse_macro_input!(item);

    let attr = AttrItem::from_list(&*attr).unwrap_or_else(|error| {
        abort!("{}", error);
    });
    let mut binder = Binder::new(attr.config());
    let output = binder.expand(attr, item);
    output.into()
}

/**
# A macro to derive [rquickjs::FromJs] for an arbitrary structured types

## Examples

### Unit struct

```
use rquickjs::FromJs;

#[derive(FromJs)]
struct MyUnit;
```

### Tuple struct

```
# use rquickjs::FromJs;
#[derive(FromJs)]
struct MyTuple(i32, String);
```

### Struct with fields

```
# use rquickjs::FromJs;
#[derive(FromJs)]
struct MyStruct {
    int: i32,
    text: String,
    #[quickjs(skip)]
    skipped: bool,
}
```

### Externally tagged enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
enum Enum {
    A(f32),
    B { s: String },
    C,
}
```

### Internally tagged enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(tag = "tag")]
enum Enum {
    A(f32),
    B { s: String },
    C,
}
```

### Adjacently tagged enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(tag = "type", content = "data")]
enum Enum {
    A(f32),
    B { s: String },
    C,
}
```

### Untagged unit enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(untagged)]
enum MyEnum {
    A,
    B,
}
```

### Untagged unit enum with discriminant

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(untagged)]
enum MyEnum {
    A = 1,
    B = 2,
}
```

### Externally tagged tuple enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Adjacently tagged tuple enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(tag, content = "data")]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Untagged tuple enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(untagged)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Internally tagged enum with fields

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

### Untagged enum with fields

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(untagged)]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

 */
#[proc_macro_error]
#[proc_macro_derive(FromJs, attributes(quickjs))]
pub fn from_js(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input);
    let input = DataType::from_derive_input(&input).unwrap_or_else(|error| {
        abort!(input.ident.span(), "FromJs deriving error: {}", error);
    });
    let config = input.config();
    let binder = FromJs::new(config);
    let output = binder.expand(&input);
    output.into()
}

/**
# A macro to derive [rquickjs::IntoJs] for an arbitrary structured types

## Examples

### Unit struct

```
use rquickjs::IntoJs;

#[derive(IntoJs)]
struct MyUnit;
```

### Tuple struct

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
struct MyTuple(i32, String);
```

### Struct with fields

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
struct MyStruct {
    int: i32,
    text: String,
}
```

### Untagged unit enum

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(untagged)]
enum MyEnum {
    Foo,
    Bar,
}
```

### Untagged unit enum with discriminant

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(untagged)]
#[repr(i32)]
enum MyEnum {
    Foo = 1,
    Bar = 2,
}
```

### Externally tagged tuple enum

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Adjacently tagged tuple enum

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(tag, content = "data")]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Untagged tuple enum

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(untagged)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Internally tagged enum with fields

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

### Untagged enum with fields

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(untagged)]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

 */
#[proc_macro_error]
#[proc_macro_derive(IntoJs, attributes(quickjs))]
pub fn into_js(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input);
    let input = DataType::from_derive_input(&input).unwrap_or_else(|error| {
        abort!(input.ident.span(), "IntoJs deriving error: {}", error);
    });
    let config = input.config();
    let binder = IntoJs::new(config);
    let output = binder.expand(&input);
    output.into()
}
