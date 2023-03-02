use super::{Error, Result};
use darling::FromMeta;
use proc_macro2::TokenStream;
use syn::{AttributeArgs, ItemEnum, ItemStruct};

#[derive(Default, FromMeta)]
#[darling(default)]
struct JsClassOptions {
    rename: Option<String>,
    frozen: bool,
}

pub(crate) fn impl_struct(attr: AttributeArgs, struct_: ItemStruct) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    todo!()
}

pub(crate) fn impl_enum(attr: AttributeArgs, enum_: ItemEnum) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    todo!()
}
