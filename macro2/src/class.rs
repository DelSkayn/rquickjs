use super::Result;
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{AttributeArgs, Field, Fields, ItemEnum, ItemStruct, Type};

#[derive(Default, FromMeta)]
#[darling(default)]
struct JsClassOptions {
    rename: Option<String>,
    frozen: bool,
    no_refs: bool,
}

// Generate HasRefs for `bar: Baz`
fn impl_struct_has_refs_field(idx: usize, field: &Field) -> Result<TokenStream> {
    let ty = &field.ty;
    let tok_impl = if let Some(ref name) = field.ident {
        quote! {
            if <#ty>::contains_ref() {
                self.#name.mark_refs(marker)
            }
        }
    } else {
        quote! {
            if <#ty>::contains_ref() {
                self.#idx.mark_refs(marker)
            }
        }
    };
    Ok(tok_impl)
}

// Generate HasRefs for `struct foo { bar: Baz }`
fn impl_struct_has_refs(struct_: &ItemStruct) -> Result<TokenStream> {
    let mark_refs_impl = match struct_.fields {
        Fields::Unit => vec![],
        Fields::Named(ref named) => named
            .named
            .iter()
            .enumerate()
            .map(|(idx, field)| impl_struct_has_refs_field(idx, field))
            .try_fold(Vec::new(), |mut acc, item| {
                acc.push(item?);
                Result::Ok(acc)
            })?,
        Fields::Unnamed(ref unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(idx, field)| impl_struct_has_refs_field(idx, field))
            .try_fold(Vec::new(), |mut acc, item| {
                acc.push(item?);
                Result::Ok(acc)
            })?,
    };

    let contains_impl = struct_.fields.iter().map(|field| {
        let ty = &field.ty;
        quote! { <#ty>::contains_ref() }
    });

    let ty_name = &struct_.ident;

    let res = quote! {
        impl rquickjs::HasRefs for #ty_name {
            fn contains_ref() -> bool
                where Self: Sized
            {
                use rquickjs::HasRefs;
                false #( || #contains_impl )*
            }

            fn mark_refs(&self, marker: &rquickjs::RefsMarker){
                #( #mark_refs_impl)*
            }
        }
    };
    Ok(res)
}

pub(crate) fn impl_struct(attr: AttributeArgs, struct_: ItemStruct) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    let has_refs_impl = if !options.no_refs {
        impl_struct_has_refs(&struct_)?
    } else {
        TokenStream::new()
    };
    let res = quote! {
        #struct_
        #has_refs_impl
    };
    Ok(res)
}

pub(crate) fn impl_enum(attr: AttributeArgs, enum_: ItemEnum) -> Result<TokenStream> {
    let options = JsClassOptions::from_list(&attr)?;
    todo!()
}
