use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use syn::ItemStruct;

use crate::crate_ident;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

pub(crate) fn derive(attr: AttrItem, item: ItemStruct) -> TokenStream {
    let ItemStruct {
        ref ident,
        ref generics,
        ..
    } = item;

    let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
    let name = format!("{}", ident);
    let name = Literal::string(&name);

    let mutable = if attr.freeze {
        quote!(#lib_crate::class::Readable)
    } else {
        quote!(#lib_crate::class::Writable)
    };

    quote! {
        #item

        impl #generics #lib_crate ::class::JsClass for #ident #generics{
            const NAME: &'static str = #name;

            type Mutable = #mutable;

            type Outlive<'a> = #ident;

            fn class_id() -> &'static #lib_crate::class::ClassId{
                static ID: #lib_crate::class::ClassId =  #lib_crate::class::ClassId::new();
                &ID
            }

            fn prototype<'js>(ctx: Ctx<'js>) -> #lib_crate::Result<Option<Object<'js>>>{
                Ok(Some(#lib_crate::Object::new(ctx)?))
            }
        }
    }
}
