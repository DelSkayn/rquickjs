use crate::Result;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{ItemEnum, ItemStruct, LitStr};

use super::JsClassOptions;

pub(super) fn impl_struct(options: &JsClassOptions, struct_: &ItemStruct) -> Result<TokenStream> {
    let js_name = options
        .rename
        .clone()
        .unwrap_or_else(|| format!("{}", struct_.ident));
    let js_name = LitStr::new(&js_name, Span::call_site());

    let name = struct_.ident;

    let res = quote! {
        impl trait rquickjs::ClassDef for #name {
            const CLASS_NAME = #js_name;

            unsafe fn class_id() -> &'static mut rquickjs::ClassId{
                static mut ID: rquickjs::ClassId = ClassId::new();

            }
        }
    };

    todo!()
}

pub(super) fn impl_enum(enum_: &ItemEnum) -> Result<TokenStream> {
    todo!()
}
