use attrs::OptionList;
use class::ClassOption;
use function::FunctionOption;
use methods::ImplOption;
use module::ModuleOption;
use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::{abort, proc_macro_error};
use syn::{parse_macro_input, DeriveInput, Item};

#[cfg(test)]
macro_rules! assert_eq_tokens {
    ($actual:expr, $expected:expr) => {
        let actual = $actual.to_string();
        let expected = $expected.to_string();
        difference::assert_diff!(&actual, &expected, " ", 0);
    };
}

mod attrs;
mod class;
mod common;
mod embed;
mod fields;
mod function;
mod methods;
mod module;
mod trace;

/// An attribute for implementing [`JsClass`](rquickjs_core::class::JsClass`) for a Rust type.
///
/// # Attribute options
///
/// The attribute has a number of options for configuring the generated trait implementation. These
/// attributes can be passed to the `class` attribute as an argument: `#[class(rename =
/// "AnotherName")]` or with a separate `qjs` attribute on the struct item: `#[qjs(rename =
/// "AnotherName")]`. A option which is a Flag can be set just by adding the attribute:
/// `#[qjs(flag)]` or by setting it to specific boolean value: `#[qjs(flag = true)]`.
///
/// | **Option**   | **Value** | **Description**                                                                                                                                                                         |
/// |--------------|-----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `crate`      | String    | Changes the name from which the attribute tries to use rquickjs types. Use when the name behind which the rquickjs crate is declared is not properly resolved by the macro.             |
/// | `rename`     | String    | Changes the name of the implemented class on the JavaScript side.                                                                                                                       |
/// | `rename_all` | Casing    | Converts the case of all the fields of this struct which have implement accessors. Can be one of `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`,`snake_case`, or `SCREAMING_SNAKE` |
/// | `frozen`     | Flag      | Changes the class implementation to only allow borrowing immutably.  Trying to borrow mutably will result in an error.                                                                  |
///
/// # Field options
///
/// The fields of a struct (doesn't work on enums) can also tagged with an attribute to, for
/// example make the fields accessible from JavaScript. These attributes are all in the form of
/// `#[qjs(option = value)]`.
///
/// | **Option**     | **Value** | **Description**                                                                         |
/// |----------------|-----------|-----------------------------------------------------------------------------------------|
/// | `get`          | Flag      | Creates a getter for this field, allowing read access to the field from JavaScript.     |
/// | `set`          | Flag      | Creates a setter for this field, allowing write access to the field from JavaSccript.   |
/// | `enumerable`   | Flag      | Makes the field, if it has a getter or setter, enumerable in JavaScript.                |
/// | `configurable` | Flag      | Makes the field, if it has a getter or setter, configurable in JavaScript.              |
/// | `skip_trace`   | Flag      | Skips the field deriving the `Trace` trait.                                             |
/// | `rename`       | String    | Changes the name of the field getter and/or setter to the specified name in JavaScript. |
///
///
/// # Example
/// ```
/// use rquickjs::{class::Trace, CatchResultExt, Class, Context, Object, Runtime};
///
/// /// Implement JsClass for TestClass.
/// /// This allows passing any instance of TestClass straight to JavaScript.
/// /// It is command to also add #[derive(Trace)] as all types which implement JsClass need to
/// /// also implement trace.
/// #[derive(Trace)]
/// #[rquickjs::class(rename_all = "camelCase")]
/// pub struct TestClass<'js> {
///     /// These attribute make the accessible from JavaScript with getters and setters.
///     /// As we used `rename_all = "camelCase"` in the attribute it will be called `innerObject`
///     /// on the JavaScript side.
///     #[qjs(get, set)]
///     inner_object: Object<'js>,
///
///     /// This works for any value which implements `IntoJs` and `FromJs` and is clonable.
///     #[qjs(get, set)]
///     some_value: u32,
///     /// Make a field enumerable.
///     #[qjs(get, set, enumerable)]
///     another_value: u32,
/// }
///
/// pub fn main() {
///     let rt = Runtime::new().unwrap();
///     let ctx = Context::full(&rt).unwrap();
///
///     ctx.with(|ctx| {
///         /// Create an insance of a JsClass
///         let cls = Class::instance(
///             ctx.clone(),
///             TestClass {
///                 inner_object: Object::new(ctx.clone()).unwrap(),
///                 some_value: 1,
///                 another_value: 2,
///             },
///         )
///         .unwrap();
///         /// Pass it to JavaScript
///         ctx.globals().set("t", cls.clone()).unwrap();
///         ctx.eval::<(), _>(
///             r#"
///             // use the actual value.
///             if(t.someValue !== 1){
///                 throw new Error(1)
///             }"#
///         ).unwrap();
///     })
/// }
/// ```

#[proc_macro_attribute]
#[proc_macro_error]
pub fn class(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let options = parse_macro_input!(attr as OptionList<ClassOption>);
    let item = parse_macro_input!(item as Item);
    TokenStream1::from(class::expand(options, item))
}

/// A attribute for implementing `IntoJsFunc` for a certain function.
///
/// Using this attribute allows a wider range of functions to be used as callbacks from JavaScript
/// then when you use closures or the functions for which the proper traits are already
/// implemented..
///
#[proc_macro_attribute]
#[proc_macro_error]
pub fn function(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let options = parse_macro_input!(attr as OptionList<FunctionOption>);
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Fn(func) => function::expand(options, func).into(),
        item => {
            abort!(item, "#[function] macro can only be used on functions")
        }
    }
}

/// A attribute for implementing methods for a class.
///
/// This attribute can be added to a impl block which implements methods for a type which uses the
/// [`macro@class`] attribute to derive [`JsClass`](rquickjs_core::class::JsClass).
///
/// # Limitations
/// Due to limitations in the Rust type system this attribute can be used on only one impl block
/// per type.
///
/// # Attribute options
///
/// The attribute has a number of options for configuring the generated trait implementation. These
/// attributes can be passed to the `methods` attribute as an argument: `#[methods(rename =
/// "AnotherName")]` or with a separate `qjs` attribute on the impl item: `#[qjs(rename =
/// "AnotherName")]`. A option which is a Flag can be set just by adding the attribute:
/// `#[qjs(flag)]` or by setting it to specific boolean value: `#[qjs(flag = true)]`.
///
/// | **Option**   | **Value** | **Description**                                                                                                                                                                         |
/// |--------------|-----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `crate`      | String    | Changes the name from which the attribute tries to use rquickjs types. Use when the name behind which the rquickjs crate is declared is not properly resolved by the macro.             |
/// | `rename`     | String    | Changes the name of the implemented class on the JavaScript side.                                                                                                                       |
/// | `rename_all` | Casing    | Converts the case of all the fields of this struct which have implement accessors. Can be one of `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`,`snake_case`, or `SCREAMING_SNAKE` |
///
///
/// # Item options
///
/// Each item of the impl block can also tagged with an attribute to change the resulting derived method definition.
/// These attributes are all in the form of `#[qjs(option = value)]`.
///
/// | **Option**     | **Value**                                                         | **Description**                                                                                 |
/// |----------------|-------------------------------------------------------------------|-------------------------------------------------------------------------------------------------|
/// | `get`          | Flag                                                              | Makes this method a getter for a field of the same name.                                        |
/// | `set`          | Flag                                                              | Makes this method a setter for a field of the same name.                                        |
/// | `enumerable`   | Flag                                                              | Makes the method, if it is a getter or setter, enumerable in JavaScript.                        |
/// | `configurable` | Flag                                                              | Makes the method, if it is a getter or setter, configurable in JavaScript.                      |
/// | `rename`       | String or [`PredefinedAtom`](rquickjs_core::atom::PredefinedAtom) | Changes the name of the field getter and/or setter to the specified name in JavaScript.         |
/// | `static`       | Flag                                                              | Makes the method a static method i.e. defined on the type constructor instead of the prototype. |
/// | `constructor`  | Flag                                                              | Marks this method a the constructor for this type.                                              |
/// | `skip`         | Flag                                                              | Skips defining this method on the JavaScript class.                                             |
///
/// # Example
/// ```
/// use rquickjs::{
///     atom::PredefinedAtom, class::Trace, prelude::Func, CatchResultExt, Class, Context, Ctx,
///     Object, Result, Runtime,
/// };
///
/// #[derive(Trace)]
/// #[rquickjs::class]
/// pub struct TestClass {
///     value: u32,
///     another_value: u32,
/// }
///
/// #[rquickjs::methods]
/// impl TestClass {
///     /// Marks a method as a constructor.
///     /// This method will be used when a new TestClass object is created from JavaScript.
///     #[qjs(constructor)]
///     pub fn new(value: u32) -> Self {
///         TestClass {
///             value,
///             another_value: value,
///         }
///     }
///
///     /// Mark a function as a getter.
///     /// The value of this function can be accessed as a field.
///     /// This function is also renamed to value
///     #[qjs(get, rename = "value")]
///     pub fn get_value(&self) -> u32 {
///         self.value
///     }
///
///     /// Mark a function as a setter.
///     /// The value of this function can be set as a field.
///     /// This function is also renamed to value
///     #[qjs(set, rename = "value")]
///     pub fn set_value(&mut self, v: u32) {
///         self.value = v
///     }
///
///     /// Mark a function as a enumerable gettter.
///     #[qjs(get, rename = "anotherValue", enumerable)]
///     pub fn get_another_value(&self) -> u32 {
///         self.another_value
///     }
///
///     #[qjs(set, rename = "anotherValue", enumerable)]
///     pub fn set_another_value(&mut self, v: u32) {
///         self.another_value = v
///     }
///
///     /// Marks a function as static. It will be defined on the constructor object instead of the
///     /// Class prototype.
///     #[qjs(static)]
///     pub fn compare(a: &Self, b: &Self) -> bool {
///         a.value == b.value && a.another_value == b.another_value
///     }
///
///     /// All functions declared in this impl block will be defined on the prototype of the
///     /// class. This attributes allows you to skip certain functions.
///     #[qjs(skip)]
///     pub fn inner_function(&self) {}
///
///     /// Functions can also be renamed to specific symbols. This allows you to make an Rust type
///     /// act like an iteratable value for example.
///     #[qjs(rename = PredefinedAtom::SymbolIterator)]
///     pub fn iterate<'js>(&self, ctx: Ctx<'js>) -> Result<Object<'js>> {
///         let res = Object::new(ctx)?;
///
///         res.set(
///             PredefinedAtom::Next,
///             Func::from(|ctx: Ctx<'js>| -> Result<Object<'js>> {
///                 let res = Object::new(ctx)?;
///                 res.set(PredefinedAtom::Done, true)?;
///                 Ok(res)
///             }),
///         )?;
///         Ok(res)
///     }
/// }
///
/// pub fn main() {
///     let rt = Runtime::new().unwrap();
///     let ctx = Context::full(&rt).unwrap();
///
///     ctx.with(|ctx| {
///         /// Define the class constructor on the globals object.
///         Class::<TestClass>::define(&ctx.globals()).unwrap();
///         ctx.eval::<(), _>(
///             r#"
///             let nv = new TestClass(5);
///             if(nv.value !== 5){
///                 throw new Error('invalid value')
///             }
///         "#,
///         ).catch(&ctx).unwrap();
///     });
/// }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn methods(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let options = parse_macro_input!(attr as OptionList<ImplOption>);
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Impl(item) => methods::expand(options, item).into(),
        item => {
            abort!(item, "#[methods] macro can only be used on impl blocks")
        }
    }
}

/// An attribute which generates code for exporting a module to Rust.
///
/// Any supported item inside the module which is marked as `pub` will be exported as a JavaScript value.
/// Different items result in different JavaScript values.
/// The supported items are:
///
/// - `struct` and `enum` items. These will be exported as JavaScript
/// classes with their constructor exported as a function from the module.
/// - `fn` items, these will be exported as JavaScript functions.
/// - `use` items, the types which are reexported with `pub` will be handled just like `struct` and
/// `enum` items defined inside the module. The name of the class can be adjusted by renaming the
/// reexport with `as`.
/// - `const` and `static` items, these items will be exported as values with the same name.
///
/// # Attribute options
///
/// The attribute has a number of options for configuring the generated trait implementation. These
/// attributes can be passed to the `module` attribute as an argument: `#[module(rename =
/// "AnotherName")]` or with a separate `qjs` attribute on the impl item: `#[qjs(rename =
/// "AnotherName")]`. A option which is a Flag can be set just by adding the attribute:
/// `#[qjs(flag)]` or by setting it to specific boolean value: `#[qjs(flag = true)]`.
///
/// | **Option**     | **Value** | **Description**                                                                                                                                                                        |
/// |----------------|-----------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `crate`        | String    | Changes the name from which the attribute tries to use rquickjs types. Use when the name behind which the rquickjs crate is declared is not properly resolved by the macro.            |
/// | `rename`       | String    | Changes the name of the implemented module on the JavaScript side.                                                                                                                     |
/// | `rename_vars`  | Casing    | Alters the name of all items exported as JavaScript values by changing the case.  Can be one of `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`,`snake_case`, or `SCREAMING_SNAKE` |
/// | `rename_types` | Casing    | Alters the name of all items exported as JavaScript classes by changing the case. Can be one of `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`,`snake_case`, or `SCREAMING_SNAKE` |
/// | `prefix`       | String    | The module will be implemented for a new type with roughly the same name as the Rust module with a prefix added. This changes the prefix which will be added. Defaults to `js_`        |
///
/// # Item options
///
/// The attribute also has a number of options for changing the resulting generated module
/// implementation for specific items.
/// These attributes are all in the form of `#[qjs(option = value)]`.
///
/// | **Option** | **Value** | **Item Type**  | **Description**                                                                                                                                                                                            |
/// |------------|-----------|----------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `skip`     | Flag      | All            | Skips exporting this item from the JavaScript module.                                                                                                                                                      |
/// | `rename`   | String    | All except use | Change the name from which this value is exported.                                                                                                                                                         |
/// | `declare`  | Flag      | Functions Only | Marks this function as the declaration function. This function will be called when the module is declared allowing for exporting items which otherwise are difficult to export using the attribute.        |
/// | `evaluate` | Flag      | Functions Only | Marks this function as the evaluation function. This function will be called when the module is being evaluated allowing for exporting items which otherwise are difficult to export using the attribute.  |
///
/// # Example
///
/// ```
/// use rquickjs::{CatchResultExt, Context, Module, Runtime};
///
/// /// A class which will be exported from the module.
/// #[derive(rquickjs::class::Trace)]
/// #[rquickjs::class]
/// pub struct Test {
///     foo: u32,
/// }
///
/// #[rquickjs::methods]
/// impl Test {
///     #[qjs(constructor)]
///     pub fn new() -> Test {
///         Test { foo: 3 }
///     }
/// }
///
/// impl Default for Test {
///     fn default() -> Self {
///         Self::new()
///     }
/// }
///
/// #[rquickjs::module(rename_vars = "camelCase")]
/// mod test_mod {
///     /// Imports and other declarations which aren't `pub` won't be exported.
///     use rquickjs::Ctx;
///
///     /// You can even use `use` to export types from outside.
///     ///
///     /// Note that this tries to export the type, not the value,
///     /// So this won't work for functions.
///     ///
///     /// By using `as` you can change under which name the constructor is exported.
///     /// The below type will exported as `RenamedTest`.
///     pub use super::Test as RenamedTest;
///
///     /// A class which will be exported from the module under the name `FooBar`.
///     #[derive(rquickjs::class::Trace)]
///     #[rquickjs::class(rename = "FooBar")]
///     pub struct Test2 {
///         bar: u32,
///     }
///
///     /// Implement methods for the class like normal.
///     #[rquickjs::methods]
///     impl Test2 {
///         /// A constructor is required for exporting types.
///         #[qjs(constructor)]
///         pub fn new() -> Test2 {
///             Test2 { bar: 3 }
///         }
///     }
///
///     impl Default for Test2 {
///         fn default() -> Self {
///             Self::new()
///         }
///     }
///
///     /// Two variables exported as `aConstValue` and `aStaticValue` because of the `rename_all` attr.
///     pub const A_CONST_VALUE: f32 = 2.0;
///     pub static A_STATIC_VALUE: f32 = 2.0;
///
///     /// If your module doesn't quite fit with how this macro exports you can manually export from
///     /// the declare and evaluate functions.
///     #[qjs(declare)]
///     pub fn declare(declare: &rquickjs::module::Declarations) -> rquickjs::Result<()> {
///         declare.declare("aManuallyExportedValue")?;
///         Ok(())
///     }
///
///     #[qjs(evaluate)]
///     pub fn evaluate<'js>(
///         _ctx: &Ctx<'js>,
///         exports: &rquickjs::module::Exports<'js>,
///     ) -> rquickjs::Result<()> {
///         exports.export("aManuallyExportedValue", "Some Value")?;
///         Ok(())
///     }
///
///     /// You can also export functions.
///     #[rquickjs::function]
///     pub fn foo() -> u32 {
///         1 + 1
///     }
///
///     /// You can make items public but not export them to JavaScript by adding the skip attribute.
///     #[qjs(skip)]
///     pub fn ignore_function() -> u32 {
///         2 + 2
///     }
/// }
///
/// fn main() {
///     assert_eq!(test_mod::ignore_function(), 4);
///     let rt = Runtime::new().unwrap();
///     let ctx = Context::full(&rt).unwrap();
///
///     ctx.with(|ctx| {
///          // These modules are declared like normal with the declare_def function.
///          // The name of the module is js_ + the name of the module. This prefix can be changed
///          // by writing for example `#[rquickjs::module(prefix = "prefix_")]`.
///          Module::declare_def::<js_test_mod, _>(ctx.clone(), "test").unwrap();
///          let _ = Module::evaluate(
///              ctx.clone(),
///              "test2",
///              r"
///              import { RenamedTest, foo,aManuallyExportedValue, aConstValue, aStaticValue, FooBar } from 'test';
///              if (foo() !== 2){
///                  throw new Error(1);
///              }
///              "
///          )
///          .catch(&ctx)
///          .unwrap();
///      })
/// }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn module(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let options = parse_macro_input!(attr as OptionList<ModuleOption>);
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Mod(item) => module::expand(options, item).into(),
        item => {
            abort!(item, "#[module] macro can only be used on modules")
        }
    }
}

/// A macro for auto deriving the trace trait.
#[proc_macro_derive(Trace, attributes(qjs))]
#[proc_macro_error]
pub fn trace(stream: TokenStream1) -> TokenStream1 {
    let derive_input = parse_macro_input!(stream as DeriveInput);
    trace::expand(derive_input).into()
}

/// A macro for embedding JavaScript code into a binary.
///
/// Compiles a JavaScript module to bytecode and then compiles the resulting bytecode into the
/// binary. Each file loaded is turned into its own module. The macro takes a list of paths to
/// files to be compiled into a module with an option name. Module paths are relative to the crate
/// manifest file.
///
/// # Usage
///
/// ```
/// use rquickjs::{embed, loader::Bundle, CatchResultExt, Context, Module, Runtime};
///
/// /// load the `my_module.js` file and name it myModule
/// static BUNDLE: Bundle = embed! {
///     "myModule": "my_module.js",
/// };
///
/// fn main() {
///     let rt = Runtime::new().unwrap();
///     let ctx = Context::full(&rt).unwrap();
///
///     rt.set_loader(BUNDLE, BUNDLE);
///     ctx.with(|ctx| {
///         Module::evaluate(
///             ctx.clone(),
///             "testModule",
///             r#"
///             import { foo } from 'myModule';
///             if(foo() !== 2){
///                 throw new Error("Function didn't return the correct value");
///             }
///         "#,
///         )
///         .unwrap()
///         .finish::<()>()
///         .catch(&ctx)
///         .unwrap();
///     })
/// }
/// ```
#[proc_macro_error]
#[proc_macro]
pub fn embed(item: TokenStream1) -> TokenStream1 {
    let embed_modules: embed::EmbedModules = parse_macro_input!(item);
    embed::embed(embed_modules).into()
}
