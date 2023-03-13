use crate::Result;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemEnum, ItemStruct, LitStr};

use super::JsClassOptions;

pub(super) fn impl_struct(options: &JsClassOptions, struct_: &ItemStruct) -> Result<TokenStream> {
    let js_name = options
        .rename
        .clone()
        .unwrap_or_else(|| format!("{}", struct_.ident));
    let js_name = LitStr::new(&js_name, Span::call_site());

    let name = &struct_.ident;

    let res = quote! {
        impl rquickjs::ClassDef for #name {
            const CLASS_NAME: &'static str = #js_name;

            fn class_id() -> &'static rquickjs::ClassId{
                static ID: rquickjs::ClassId = rquickjs::ClassId::new();
                &ID
            }

             const HAS_PROTO: bool = true;
             fn init_proto<'js>(ctx: rquickjs::Ctx<'js>, proto: &rquickjs::Object<'js>) -> Result<()> {
                 (&rquickjs::impl_::MethodImplementor::<#name>::new()).init_proto(ctx,proto)
             }

             // With statics
             const HAS_STATIC: bool = true;
             fn init_static<'js>(ctx: rquickjs::Ctx<'js>, ctor: &rquickjs::Object<'js>) -> Result<()> {
                 Ok(())
             }

        }
    };
    Ok(res)
}

pub(super) fn impl_enum(options: &JsClassOptions, enum_: &ItemEnum) -> Result<TokenStream> {
    let js_name = options
        .rename
        .clone()
        .unwrap_or_else(|| format!("{}", enum_.ident));
    let js_name = LitStr::new(&js_name, Span::call_site());

    let name = &enum_.ident;

    let res = quote! {
        impl rquickjs::ClassDef for #name {
            const CLASS_NAME: &'static str = #js_name;

            fn class_id() -> &'static rquickjs::ClassId{
                static ID: rquickjs::ClassId = rquickjs::ClassId::new();
                &ID
            }

             const HAS_PROTO: bool = true;
             fn init_proto<'js>(ctx: rquickjs::Ctx<'js>, proto: &rquickjs::Object<'js>) -> Result<()> {
                 (&rquickjs::impl_::MethodImplementor::<#name>::new()).init_proto(ctx,proto)
             }

             // With statics
             const HAS_STATIC: bool = true;
             fn init_static<'js>(ctx: rquickjs::Ctx<'js>, ctor: &rquickjs::Object<'js>) -> Result<()> {
                 Ok(())
             }
        }
    };
    Ok(res)
}
