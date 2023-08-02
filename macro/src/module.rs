use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, abort_call_site};
use syn::ItemMod;

use crate::common::Case;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    rename: Option<String>,
    rename_all: Option<Case>,
}

pub struct JsModule {
    name: Ident,
}

// - exports
//
// static / const: Variable
// fn: Function / Calculated variable,
// struct / enum: Class,
// type: Class
//
// impl: class body,
//
// -  missing
//
// - unused
//
// use: Rexport?
//
// - not allowed
//
// union,extern
//
//
pub(crate) fn expand(_attr: AttrItem, item: ItemMod) -> TokenStream {
    let ItemMod {
        content, unsafety, ..
    } = item;

    if let Some(unsafe_) = unsafety {
        abort!(unsafe_, "unsafe modules are not supported");
    }

    let Some(content_) = content else {
        abort_call_site!(
            "The `module` macro can only be applied to modules with a definition in the same file."
        )
    };

    abort_call_site!("Not yet implemented")
}
