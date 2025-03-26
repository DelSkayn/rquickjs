use rquickjs::{CatchResultExt, Context, Module, Runtime};

#[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
#[rquickjs::class]
pub struct Test {
    foo: u32,
}

#[rquickjs::methods]
impl Test {
    #[qjs(constructor)]
    pub fn new() -> Test {
        Test { foo: 3 }
    }
}

impl Default for Test {
    fn default() -> Self {
        Self::new()
    }
}

#[rquickjs::module(rename_vars = "camelCase")]
mod test_mod {
    /// Imports and other declarations which aren't `pub` won't be exported.
    use rquickjs::Ctx;

    /// You can even use `use` to export types from outside.
    ///
    /// Note that this tries to export the type, not the value,
    /// So this won't work for functions.
    pub use super::Test;

    #[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
    #[rquickjs::class(rename = "FooBar")]
    pub struct Test2 {
        bar: u32,
    }

    /// Implement methods for the class like normal.
    #[rquickjs::methods]
    impl Test2 {
        /// A constructor is required for exporting types.
        #[qjs(constructor)]
        pub fn new() -> Test2 {
            Test2 { bar: 3 }
        }
    }

    impl Default for Test2 {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Two variables exported as `aConstValue` and `aStaticValue` because of the `rename_all` attr.
    pub const A_CONST_VALUE: f32 = 2.0;
    pub static A_STATIC_VALUE: f32 = 2.0;

    /// If our module doesn't quite fit with how this macro exports you can manually export from
    /// the declare and evaluate functions.
    #[qjs(declare)]
    pub fn declare(declare: &rquickjs::module::Declarations) -> rquickjs::Result<()> {
        declare.declare("aManuallyExportedValue")?;
        Ok(())
    }

    #[qjs(evaluate)]
    pub fn evaluate<'js>(
        _ctx: &Ctx<'js>,
        exports: &rquickjs::module::Exports<'js>,
    ) -> rquickjs::Result<()> {
        exports.export("aManuallyExportedValue", "Some Value")?;
        Ok(())
    }

    /// You can also export functions.
    #[rquickjs::function]
    pub fn foo() -> u32 {
        1 + 1
    }

    /// You can make items public but not export them to JavaScript by adding the skip attribute.
    #[qjs(skip)]
    pub fn ignore_function() -> u32 {
        2 + 2
    }
}

pub fn main() {
    assert_eq!(test_mod::ignore_function(), 4);
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        Module::declare_def::<js_test_mod, _>(ctx.clone(), "test").unwrap();
        let _ = Module::evaluate(
            ctx.clone(),
            "test2",
            r"
            import { foo,aManuallyExportedValue, aConstValue, aStaticValue, FooBar } from 'test';
            if (foo() !== 2){
                throw new Error(1);
            }
            if (aManuallyExportedValue !== 'Some Value'){
                throw new Error(2);
            }
            if(aConstValue !== 2){
                throw new Error(3);
            }
            if(aStaticValue !== 2){
                throw new Error(4);
            }
            if(aStaticValue !== 2){
                throw new Error(4);
            }
            let test = new FooBar();
        ",
        )
        .catch(&ctx)
        .unwrap();
    })
}
