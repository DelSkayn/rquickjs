use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, abort_call_site};
use syn::{parse::Parse, ItemMod};

use crate::{common::Case, class};


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
pub(crate) fn expand(item: ItemMod) -> TokenStream {
    let ItemMod {
        content, unsafety, ..
    } = item;

    if let Some(unsafe_) = unsafety {
        abort!(unsafe_, "unsafe modules are not supported");
    }

    let Some((_, items)) = content else {
        abort_call_site!(
            "The `module` macro can only be applied to modules with a definition in the same file."
        )
    };

    for item in items {
        match item {
            syn::Item::Const(_) => todo!(),
            syn::Item::Enum(_) => todo!(),
            syn::Item::ExternCrate(_) => todo!(),
            syn::Item::Fn(_) => todo!(),
            syn::Item::ForeignMod(_) => todo!(),
            syn::Item::Impl(_) => todo!(),
            syn::Item::Macro(_) => todo!(),
            syn::Item::Mod(_) => todo!(),
            syn::Item::Static(_) => todo!(),
            syn::Item::Struct(s) => {
                class::AttrItem::from_meta( s.attrs
            }
            syn::Item::Trait(_)
            | syn::Item::TraitAlias(_)
            | syn::Item::Type(_)
            | syn::Item::Union(_)
            | syn::Item::Use(_)
            | syn::Item::Verbatim(_) => {}
        }
    }

    abort_call_site!("Not yet implemented")
}
