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

/// An attribute for implementing JsClass for a rust type.
///
/// # Example
/// ```
/// use rquickjs::{class::Trace, CatchResultExt, Class, Context, Object, Runtime};
///
/// /// Implement JsClass for TestClass.
/// /// This allows passing any instance of TestClass straight to javascript.
/// /// It is command to also add #[derive(Trace)] as all types which implement JsClass need to
/// /// also implement trace.
/// #[derive(Trace)]
/// #[rquickjs::class(rename_all = "camelCase")]
/// pub struct TestClass<'js> {
///     /// These attribute make the accessible from javascript with getters and setters.
///     /// As we used `rename_all = "camelCase"` in the attribute it will be called `innerObject`
///     /// on the javascript side.
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
///         /// Pass it to javascript
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
/// Using this attribute allows a wider range of functions to be used as callbacks from javascript
/// then when you use closures or straight functions.
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

/// A macro for implementing methods for a class.
///
/// # Example
/// ```
/// use rquickjs::{
///     atom::PredefinedAtom, class::Trace, prelude::Func, CatchResultExt, Class, Context, Ctx, Object,
///     Result, Runtime,
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
///     /// This method will be used when
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
///     /// Functions can also be renamed to specific symbols. This allows you to make an rust type
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

/// An attribute which generates code for exporting a module to rust.
/// ```
///
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
///     pub use super::Test;
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
///     pub fn declare(declare: &mut rquickjs::module::Declarations) -> rquickjs::Result<()> {
///         declare.declare("aManuallyExportedValue")?;
///         Ok(())
///     }
///
///     #[qjs(evaluate)]
///     pub fn evaluate<'js>(
///         _ctx: &Ctx<'js>,
///         exports: &mut rquickjs::module::Exports<'js>,
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
///     /// You can make items public but not export them to javascript by adding the skip attribute.
///     #[qjs(skip)]
///     pub fn ignore_function() -> u32 {
///         2 + 2
///     }
/// }
///
///
/// fn main() {
///     assert_eq!(test_mod::ignore_function(), 4);
///     let rt = Runtime::new().unwrap();
///     let ctx = Context::full(&rt).unwrap();
///
///     ctx.with(|ctx| {
///         // These modules are declared like normal with the declare_def function.
///         // The name of the module is js_ + the name of the module. This prefix can be changed
///         // by writing for example `#[rquickjs::module(prefix = "prefix_")]`.
///         Module::declare_def::<js_test_mod, _>(ctx.clone(), "test").unwrap();
///         let _ = Module::evaluate(
///             ctx.clone(),
///             "test2",
///             r"
///             import { foo,aManuallyExportedValue, aConstValue, aStaticValue, FooBar } from 'test';
///             if (foo() !== 2){
///                 throw new Error(1);
///             }
///             "
///         )
///         .catch(&ctx)
///         .unwrap();
///     })
/// }
///
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

/// A macro for embedding javascript code into a binary.
///
/// Compiles a javascript module to bytecode and then compiles the resulting bytecode into the
/// binary. Each file loaded is turned into its own module. The macro takes a list of paths to
/// files to be compiled into a module with an option name. Module paths are relative to the crate
/// manifest file.
///
/// # Usage
///
/// ```
/// use rquickjs::{embed, loader::Bundle, CatchResultExt, Context, Runtime};
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
///         let _ = ctx
///             .clone()
///             .compile(
///                 "testModule",
///                 r#"
///             import { foo } from 'myModule';
///             if(foo() !== 2){
///                 throw new Error("Function didn't return the correct value");
///             }
///         "#,
///             )
///             .catch(&ctx)
///             .unwrap();
///     })
/// }
///
/// ```
///
///
///
#[proc_macro_error]
#[proc_macro]
pub fn embed(item: TokenStream1) -> TokenStream1 {
    let embed_modules: embed::EmbedModules = parse_macro_input!(item);
    embed::embed(embed_modules).into()
}
