use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::ItemStruct;

pub fn lib_crate() -> String {
    env!("CARGO_PKG_NAME").replace("-macro", "")
}

pub fn derive(item: ItemStruct) -> TokenStream {
    let ItemStruct {
        ref ident,
        ref generics,
        ..
    } = item;

    let lib_crate = format_ident!("{}", lib_crate());
    let name = format!("{}", ident);
    let name = Literal::string(&name);

    quote! {
        #item

        impl #generics #lib_crate ::class::JsClass for #ident #generics{
            const NAME: &'static str = #name;

            type Mutable = #lib_crate::class::Writable;

            fn class_id() -> &'static #lib_crate::class::ClassId{
                static ID: #lib_crate::class::ClassId =  #lib_crate::class::ClassId::new();
                &ID
            }
        }
    }
}
