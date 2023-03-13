use super::Result;
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{AttributeArgs, ItemEnum, ItemStruct};

mod class_def;
mod has_refs;

#[derive(Default, FromMeta)]
#[darling(default)]
struct JsClassOptions {
    rename: Option<String>,
    frozen: bool,
    no_refs: bool,
}

pub(crate) fn impl_struct(attr: AttributeArgs, struct_: ItemStruct) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    let has_refs_impl = if !options.no_refs {
        has_refs::impl_struct(&struct_)?
    } else {
        TokenStream::new()
    };
    let class_def_impl = class_def::impl_struct(&options, &struct_)?;
    let res = quote! {
        #struct_
        #class_def_impl
        #has_refs_impl
    };
    Ok(res)
}

pub(crate) fn impl_enum(attr: AttributeArgs, enum_: ItemEnum) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    let has_refs_impl = if !options.no_refs {
        has_refs::impl_enum(&enum_)?
    } else {
        TokenStream::new()
    };
    let class_def_impl = class_def::impl_enum(&options, &enum_)?;
    let res = quote! {
        #enum_
        #class_def_impl
        #has_refs_impl
    };
    Ok(res)
}
