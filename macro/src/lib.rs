#[cfg(test)]
macro_rules! abort {
    ($err:expr) => { panic!($err) };
    ($fmt:literal $($tts:tt)*) => { panic!("{}", format!($fmt $($tts)*)) };
    ($span:expr, $($tts:tt)*) => { { let _ = $span; panic!("{}", format!($($tts)*)); } };
}

#[cfg(test)]
macro_rules! error {
    ($err:expr) => { panic!($err) };
    ($fmt:literal $($tts:tt)*) => { eprintln!("{}", format!($fmt $($tts)*)) };
    ($span:expr, $($tts:tt)*) => { { let _ = $span; eprintln!("{}", format!($($tts)*)); } };
}

#[cfg(test)]
macro_rules! warning {
    ($err:expr) => { panic!($err) };
    ($fmt:literal $($tts:tt)*) => { eprintln!("{}", format!($fmt $($tts)*)) };
    ($span:expr, $($tts:tt)*) => { { let _ = $span; eprintln!("{}", format!($($tts)*)); } };
}

#[cfg(not(test))]
macro_rules! abort {
    ($($tokens:tt)*) => { proc_macro_error::abort!($($tokens)*) };
}

#[cfg(not(test))]
macro_rules! error {
    ($($tokens:tt)*) => { proc_macro_error::emit_error!($($tokens)*) };
}

#[cfg(not(test))]
macro_rules! warning {
    ($($tokens:tt)*) => { proc_macro_error::emit_warning!($($tokens)*) };
}

#[cfg(test)]
macro_rules! assert_eq_tokens {
    ($actual:expr, $expected:expr) => {
        let actual = $actual.to_string();
        let expected = $expected.to_string();
        difference::assert_diff!(&actual, &expected, " ", 0);
    };
}

mod bind;
mod config;
mod context;
mod derive;
mod embed;
mod shim;
mod utils;

use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::parse_macro_input;

use proc_macro2::{Ident, TokenStream};

use bind::*;
use config::*;
use context::*;
use derive::*;
use embed::*;
use shim::*;
use utils::*;

/**
An attribute to generate bindings easy

This macro allows register Rust constants, functions, data types and modules to use it from JavaScript.

*NOTE: To export any nested items it should be public.*

# Supported attributes

Any attributes which is enclosed by a `#[quickjs(...)]` will be interpreted by this macro to control bindings.

## Macro attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`ident = "MyModule"`__          | The name of target unit struct to export
__`public`__, __`public = "self/super/crate"`__ | Makes the target unit struct visible
__`module`__                      | Adds the [`ModuleDef`](rquickjs_core::ModuleDef) impl to use bindings as ES6 module
__`object`__                      | Adds the [`ObjectDef`](rquickjs_core::ObjectDef) impl for attaching bindings to an object
__`init`__, __`init = "js_module_init"`__     | Adds the `js_module_init` function (in particular for creating dynamically loadable modules or static libraries to use from `C`)
__`crate = "rquickjs"`__          | Allows rename `rquickjs` crate

## Module attributes

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames module to export
__`bare`__                | Exports contents of the module to the parent module instead of creating submodule (this is off by default)
__`skip`__                | Skips exporting this module
__`hide`__                | Do not output this module (bindings only)

## Constant attributes

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames constant to export
__`value`__               | Defines a property
__`writable`__            | Makes property to be writable
__`configurable`__        | Makes property to be configurable
__`enumerable`__          | Makes property to be enumerable
__`proto`__               | Sets constant or property to prototype
__`skip`__                | Skips exporting this contant
__`hide`__                | Do not output this constant (bindings only)

## Function attributes

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames function to export
__`get`__                 | Uses function as a getter for a property
__`set`__                 | Uses function as a setter for a property
__`configurable`__        | Makes property to be configurable
__`enumerable`__          | Makes property to be enumerable
__`constructor`__, __`constructor = true`__  | Forces creating contructor
__`constructor = false`__ | Disables creating contructor
__`skip`__                | Skips exporting this function
__`hide`__                | Do not output this function (bindings only)

When multiple functions is declared with same name (i.e. same `rename` attribute value) it will be overloaded. The overloading rules is dead simple, so currently you should be care to get it works.
Overloading is not supported for property getters/setters.

## Data type attributes

This attributes applies to structs and enums to use it as ES6 classes.

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames data type to export
__`has_refs`__            | Marks data which has internal refs to other JS values (requires [`HasRefs`](rquickjs_core::HasRefs) to be implemented)
__`cloneable`__           | Marks data type which implements `Clone` trait
__`skip`__                | Skips exporting this data type
__`hide`__                | Do not output this data type (bindings only)

The following traits will be implemented for data type:
- [`ClassDef`](rquickjs_core::ClassDef)
- [`IntoJs`](rquickjs_core::IntoJs)
- [`FromJs`](rquickjs_core::FromJs) if `cloneable` attribute is present

The following traits will be implemented for references to data type:
- [`IntoJs`](rquickjs_core::IntoJs) if `cloneable` attribute is present
- [`FromJs`](rquickjs_core::IntoJs)

## Data field attributes

This attributes applies to data fields to use it as a properties.

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames field to export
__`readonly`__            | Makes this field to be readonly
__`skip`__                | Skips exporting this field

## `impl` block attributes

This attributes applies to `impl` blocks to bind class methods and properties and also adding static constants and functions.

Attribute                 | Description
------------------------- | ---------------------------
__`rename = "new_name"`__ | Renames data type to export
__`has_refs`__            | Marks data which has internal refs to other JS values (requires [`HasRefs`](rquickjs_core::HasRefs) to be implemented)
__`skip`__                | Skips exporting this impl block
__`hide`__                | Do not output this impl block (bindings only)

# Examples

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

### Module with two functions which reused from another module

```
use rquickjs::{Runtime, Context, Object, bind};

mod my_math {
    pub const PI: f32 = core::f32::consts::PI;

    pub fn add2(a: f32, b: f32) -> f32 {
        a + b
    }

    pub fn mul2(a: f32, b: f32) -> f32 {
        a * b
    }
}

#[bind(object)]
mod math {
    pub use super::my_math::*;

    #[quickjs(hide)]
    pub const PI: f32 = ();

    #[quickjs(hide)]
    pub fn add2(a: f32, b: f32) -> f32 {}

    #[quickjs(hide)]
    pub fn mul2(a: f32, b: f32) -> f32 {}
}

# fn main() {
let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    let glob = ctx.globals();
    glob.init_def::<Math>().unwrap();

    let res: f32 = ctx.eval(r#"math.mul2(3, math.add2(1, 2))"#).unwrap();
    assert_eq!(res, 9.0);
});
# }
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
use rquickjs::{Runtime, Context, Promise, bind, AsyncStd};

#[bind(object)]
pub async fn sleep(msecs: u64) {
    async_std::task::sleep(
        std::time::Duration::from_millis(msecs)
    ).await;
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

rt.spawn_executor(AsyncStd);

ctx.with(|ctx| {
    ctx.globals().init_def::<Sleep>().unwrap();
});

let promise: Promise<String> = ctx.with(|ctx| {
    ctx.eval(r#"
        async function mysleep() {
            await sleep(50);
            return "ok";
        }
        mysleep()
    "#).unwrap()
});

let res = promise.await.unwrap();
assert_eq!(res, "ok");

rt.idle().await;
# }
```

### Class binding

```
use rquickjs::{bind, Runtime, Context, Error};

#[bind(object)]
#[quickjs(bare)]
mod geom {
    pub struct Point {
        // field properties
        pub x: f64,
        pub y: f64,
    }

    impl Point {
        // constructor
        pub fn new(x: f64, y: f64) -> Self {
            Self { x, y }
        }

        // instance method
        pub fn norm(&self) -> f64 {
            Self::dot(self, self).sqrt()
        }

        // instance property getter
        #[quickjs(get, enumerable)]
        pub fn xy(&self) -> (f64, f64) {
            (self.x, self.y)
        }

        // instance property setter
        #[quickjs(rename = "xy", set)]
        pub fn set_xy(&mut self, xy: (f64, f64)) {
            self.x = xy.0;
            self.y = xy.1;
        }

        // static method
        pub fn dot(a: &Point, b: &Point) -> f64 {
            a.x * b.x + a.y * b.y
        }

        // static property with getter
        #[quickjs(get)]
        pub fn zero() -> Self {
            Point { x: 0.0, y: 0.0 }
        }
    }
}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

ctx.with(|ctx| {
    ctx.globals().init_def::<Geom>().unwrap();
});

ctx.with(|ctx| {
    ctx.eval::<(), _>(r#"
        function assert(res) {
            if (!res) throw new Error("Assertion failed");
        }

        class ColorPoint extends Point {
            constructor(x, y, color) {
            super(x, y);
                this.color = color;
            }
            get_color() {
                return this.color;
            }
        }

        let pt = new Point(2, 3);
        assert(pt.x === 2);
        assert(pt.y === 3);
        pt.x = 4;
        assert(pt.x === 4);
        assert(pt.norm() == 5);
        let xy = pt.xy;
        assert(xy.length === 2);
        assert(xy[0] === 4);
        assert(xy[1] === 3);
        pt.xy = [3, 4];
        assert(pt.x === 3);
        assert(pt.y === 4);
        assert(Point.dot(pt, Point(2, 1)) == 10);

        let ptz = Point.zero;
        assert(ptz.x === 0);
        assert(ptz.y === 0);

        let ptc = new ColorPoint(2, 3, 0xffffff);
        assert(ptc.x === 2);
        assert(ptc.color === 0xffffff);
        assert(ptc.get_color() === 0xffffff);
    "#).unwrap();
});
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
An attribute to convert scripts modules into builtins

# Supported attributes

Attribute                  | Description
-------------------------- | --------------------------
__`ident = "MyModule"`__   | The name of target unit struct to export
__`public`__, __`public = "self/super/crate"`__ | Makes the target unit struct visible
__`path = "search-path"`__ | Add a paths where modules can be found
__`name = "module-name"`__ | The name of module to embed
__`perfect`__              | Use perfect hash map for embedded modules (`feature = "phf"`)
__`crate = "rquickjs"`__   | Allows rename `rquickjs` crate

# Examples

```
# use rquickjs::{embed, Runtime, Context, Module};

#[embed(path = "../examples/module-loader")]
mod script_module {}

let rt = Runtime::new().unwrap();
let ctx = Context::full(&rt).unwrap();

rt.set_loader(SCRIPT_MODULE, SCRIPT_MODULE);

ctx.with(|ctx| {
    ctx.compile("script", r#"
        import { n, s, f } from "script_module";
    "#).unwrap();
});
```

 */
#[proc_macro_error]
#[proc_macro_attribute]
pub fn embed(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let attr: AttributeArgs = parse_macro_input!(attr);
    let item = parse_macro_input!(item);

    let attr = AttrEmbed::from_list(&*attr).unwrap_or_else(|error| {
        abort!("{}", error);
    });
    let embedder = Embedder::new(attr.config());
    let output = embedder.expand(attr, item);
    output.into()
}

/**
A macro to derive [`HasRefs`](rquickjs_core::HasRefs)

# Supported attributes

## Macro attributes

Attribute                | Description
------------------------ | --------------------------
__`crate = "rquickjs"`__ | Allows rename `rquickjs` crate

## Field attributes

Attribute                | Description
------------------------ | --------------------------
__`has_refs`__           | Mark a field which has referenses

# Examples

## Struct

```
# use std::collections::HashMap;
# use rquickjs::{HasRefs, Persistent, Function, Array};

#[derive(HasRefs)]
struct Data {
    #[quickjs(has_refs)]
    lists: HashMap<String, Persistent<Array<'static>>>,
    #[quickjs(has_refs)]
    func: Persistent<Function<'static>>,
    flag: bool,
    text: String,
}
```

## Enum

```
# use std::collections::HashMap;
# use rquickjs::{HasRefs, Persistent, Function, Array};

#[derive(HasRefs)]
enum Data {
    Lists(
        #[quickjs(has_refs)]
        HashMap<String, Persistent<Array<'static>>>
    ),
    Func {
        name: String,
        #[quickjs(has_refs)]
        func: Persistent<Function<'static>>,
    },
    Flag(bool),
    Text(String),
}
```
*/
#[proc_macro_error]
#[proc_macro_derive(HasRefs, attributes(quickjs))]
pub fn has_refs(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input);
    let input = DataType::from_derive_input(&input).unwrap_or_else(|error| {
        abort!(input.ident.span(), "HasRefs deriving error: {}", error);
    });
    let config = input.config();
    let binder = HasRefs::new(config);
    let output = binder.expand(&input);
    output.into()
}

/**
A macro to derive [`FromJs`](rquickjs_core::FromJs) for an arbitrary structured types

# Supported attributes

## Macro attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename_all = "<rule>"`__       | Renames variants and fields by applying renaming rule ("lowercase", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case")
__`bound = "T: Bound"`__          | Overrides type paremters bounds
__`tag`__, __`tag = "type"`__     | Turns `enum` representation to **internally tagged**
__`content`__, __`content = "data"`__ | With `tag` turns `enum` representation to **adjacently tagged**
__`untagged`__                    | Turns `enum` representation to **untagged**
__`crate = "rquickjs"`__          | Allows rename `rquickjs` crate

The default `enum` representation is **externally tagged**.

## Variant attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a variant
__`skip`__                        | Skips this variant

If a `enum` representation is `untagged` the variants with `discriminant` will be represented as a numbers.

## Field attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a field
__`default`__, __`default = "path"`__ | Sets the default for a field
__`skip`__                        | Skips this field

# Examples

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

### Struct with fields with default values

```
# use rquickjs::FromJs;
#[derive(FromJs)]
struct MyStruct {
    #[quickjs(default)]
    int: i32,
    #[quickjs(default = "default_text")]
    text: String,
}

fn default_text() -> String {
    "hello".into()
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

### Internally tagged enum with fields with defaults

```
# use rquickjs::FromJs;
#[derive(FromJs)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo {
        x: f64,
        #[quickjs(default)]
        y: f64,
    },
    Bar {
        #[quickjs(default = "default_msg")]
        msg: String,
    },
}

fn default_msg() -> String {
    "my message".into()
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

### Generic newtype-like struct

```
# use rquickjs::FromJs;
#[derive(FromJs)]
struct Newtype<T>(pub T);
```

### Generic struct with fields

```
# use rquickjs::FromJs;
#[derive(FromJs)]
struct MyStruct<T> {
    pub tag: i32,
    pub value: T,
}
```

### Generic enum

```
# use rquickjs::FromJs;
#[derive(FromJs)]
enum MyEnum<T> {
    Foo(T),
    Bar { value: T, tag: i32 },
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
A macro to derive [`IntoJs`](rquickjs_core::IntoJs) for an arbitrary structured types

# Supported attributes

## Macro attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename_all = "<rule>"`__       | Renames variants and fields by applying renaming rule ("lowercase", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case")
__`bound = "T: Bound"`__          | Overrides type paremters bounds
__`tag`__, __`tag = "type"`__     | Turns `enum` representation to **internally tagged**
__`content`__, __`content = "data"`__ | With `tag` turns `enum` representation to **adjacently tagged**
__`untagged`__                    | Turns `enum` representation to **untagged**
__`crate = "rquickjs"`__          | Allows rename `rquickjs` crate

The default `enum` representation is **externally tagged**.

## Variant attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a variant
__`skip`__                        | Skips this variant

If a `enum` representation is `untagged` the variants with `discriminant` will be represented as a numbers.

## Field attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a field
__`default`__, __`default = "path"`__ | Sets the default for a field
__`skip_default`__                | Skip named field when default value is set
__`skip`__                        | Skips this field

# Examples

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

### Struct with fields with default values

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
struct MyStruct {
    #[quickjs(skip_default)]
    int: i32,
    #[quickjs(default = "default_text", skip_default)]
    text: String,
}

fn default_text() -> String {
    "hello".into()
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

### Internally tagged enum with fields with defaults

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo {
        x: f64,
        #[quickjs(skip_default)]
        y: f64,
    },
    Bar {
        #[quickjs(default = "default_msg", skip_default)]
        msg: String,
    },
}

fn default_msg() -> String {
    "my message".into()
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

### Generic newtype-like struct

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
struct Newtype<T>(pub T);
```

### Generic struct with fields

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
struct MyStruct<T, V> {
    pub tag: T,
    pub value: V,
}
```

### Generic enum

```
# use rquickjs::IntoJs;
#[derive(IntoJs)]
enum MyEnum<T, V> {
    Foo(V),
    Bar { value: V, tag: T },
}
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
    let output = binder.expand(&input, false);
    output.into()
}

/**
A macro to derive [`IntoJs`](rquickjs_core::IntoJs) for an arbitrary structured types when it used by reference

# Supported attributes

## Macro attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename_all = "<rule>"`__       | Renames variants and fields by applying renaming rule ("lowercase", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case")
__`bound = "for<'a> &'a T: Bound"`__          | Overrides type paremters bounds
__`tag`__, __`tag = "type"`__     | Turns `enum` representation to **internally tagged**
__`content`__, __`content = "data"`__ | With `tag` turns `enum` representation to **adjacently tagged**
__`untagged`__                    | Turns `enum` representation to **untagged**
__`crate = "rquickjs"`__          | Allows rename `rquickjs` crate

The default `enum` representation is **externally tagged**.

## Variant attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a variant
__`skip`__                        | Skips this variant

If a `enum` representation is `untagged` the variants with `discriminant` will be represented as a numbers.

## Field attributes

Attribute                         | Description
--------------------------------- | ---------------------------
__`rename = "new_name"`__         | Renames a field
__`default`__, __`default = "path"`__ | Sets the default for a field
__`skip_default`__                | Skip named field when default value is set
__`skip`__                        | Skips this field

# Examples

### Unit struct

```
use rquickjs::IntoJsByRef;

#[derive(IntoJsByRef)]
struct MyUnit;
```

### Tuple struct

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
struct MyTuple(i32, String);
```

### Struct with fields

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
struct MyStruct {
    int: i32,
    text: String,
}
```

### Struct with fields with default values

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
struct MyStruct {
    #[quickjs(skip_default)]
    int: i32,
    #[quickjs(default = "default_text", skip_default)]
    text: String,
}

fn default_text() -> String {
    "hello".into()
}
```

### Untagged unit enum

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(untagged)]
enum MyEnum {
    Foo,
    Bar,
}
```

### Untagged unit enum with discriminant

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(untagged)]
#[repr(i32)]
enum MyEnum {
    Foo = 1,
    Bar = 2,
}
```

### Externally tagged tuple enum

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Adjacently tagged tuple enum

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(tag, content = "data")]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Untagged tuple enum

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(untagged)]
enum MyEnum {
    Foo(f64, f64),
    Bar(String),
}
```

### Internally tagged enum with fields

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

### Internally tagged enum with fields with defaults

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(tag = "$")]
enum MyEnum {
    Foo {
        x: f64,
        #[quickjs(skip_default)]
        y: f64,
    },
    Bar {
        #[quickjs(default = "default_msg", skip_default)]
        msg: String,
    },
}

fn default_msg() -> String {
    "my message".into()
}
```

### Untagged enum with fields

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
#[quickjs(untagged)]
enum MyEnum {
    Foo { x: f64, y: f64 },
    Bar { msg: String },
}
```

### Generic newtype-like struct

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
struct Newtype<T>(pub T);
```

### Generic struct with fields

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
struct MyStruct<T, V> {
    pub tag: T,
    pub value: V,
}
```

### Generic enum

```
# use rquickjs::IntoJsByRef;
#[derive(IntoJsByRef)]
enum MyEnum<T, V> {
    Foo(V),
    Bar { value: V, tag: T },
}
 */
#[proc_macro_error]
#[proc_macro_derive(IntoJsByRef, attributes(quickjs))]
pub fn into_js_by_ref(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input);
    let input = DataType::from_derive_input(&input).unwrap_or_else(|error| {
        abort!(input.ident.span(), "IntoJs deriving error: {}", error);
    });
    let config = input.config();
    let binder = IntoJs::new(config);
    let output = binder.expand(&input, true);
    output.into()
}
