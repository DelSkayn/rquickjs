use darling::{export::NestedMeta, FromMeta};
use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use syn::{parse_macro_input, DeriveInput, Item};

#[cfg(test)]
macro_rules! assert_eq_tokens {
    ($actual:expr, $expected:expr) => {
        let actual = $actual.to_string();
        let expected = $expected.to_string();
        difference::assert_diff!(&actual, &expected, " ", 0);
    };
}

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
    let meta = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error().into(),
    };

    let attr = class::AttrItem::from_list(&meta).unwrap_or_else(|error| {
        abort_call_site!("{}", error);
    });

    let item = parse_macro_input!(item as Item);
    match item {
        Item::Struct(item) => TokenStream1::from(class::expand(attr, item)),
        item => {
            abort!(item, "#[class] macro can only be used on structs")
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn function(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let meta = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error().into(),
    };

    let attr = function::AttrItem::from_list(&meta).unwrap_or_else(|error| {
        abort_call_site!("{}", error);
    });

    let item = parse_macro_input!(item as Item);
    match item {
        Item::Fn(func) => function::expand(attr, func).into(),
        item => {
            abort!(item, "#[function] macro can only be used on functions")
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn methods(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let meta = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error().into(),
    };

    let attr = methods::ImplAttr::from_list(&meta).unwrap_or_else(|error| {
        abort_call_site!("{}", error);
    });

    let item = parse_macro_input!(item as Item);
    match item {
        Item::Impl(item) => methods::expand(attr, item).into(),
        item => {
            abort!(item, "#[methods] macro can only be used on impl blocks")
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn module(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let meta = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error().into(),
    };

    let attr = module::AttrItem::from_list(&meta).unwrap_or_else(|error| {
        abort_call_site!("{}", error);
    });

    let item = parse_macro_input!(item as Item);
    match item {
        Item::Mod(item) => module::expand(attr, item).into(),
        item => {
            abort!(item, "#[module] macro can only be used on a module")
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
