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

#[proc_macro_attribute]
#[proc_macro_error]
pub fn class(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let options = parse_macro_input!(attr as OptionList<ClassOption>);
    let item = parse_macro_input!(item as Item);
    TokenStream1::from(class::expand(options, item))
}

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

#[proc_macro_derive(Trace, attributes(qjs))]
#[proc_macro_error]
pub fn trace(stream: TokenStream1) -> TokenStream1 {
    let derive_input = parse_macro_input!(stream as DeriveInput);
    trace::expand(derive_input).into()
}

#[proc_macro_error]
#[proc_macro]
pub fn embed(item: TokenStream1) -> TokenStream1 {
    let embed_modules: embed::EmbedModules = parse_macro_input!(item);
    embed::embed(embed_modules).into()
}
