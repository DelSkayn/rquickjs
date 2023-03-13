use std::collections::HashSet;

use crate::Result;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{Field, Fields, Ident, Index, ItemEnum, ItemStruct, Type, Variant};

// Generate HasRefs for `bar: Baz`
fn impl_struct_field(idx: usize, container: &TokenStream, field: &Field) -> Result<TokenStream> {
    let ty = &field.ty;
    let tok_impl = if let Some(ref name) = field.ident {
        quote! {
            if <#ty>::contains_ref() {
                #container.#name.mark_refs(marker)
            }
        }
    } else {
        let idx = Index::from(idx);

        quote! {
            if <#ty>::contains_ref() {
                #container.#idx.mark_refs(marker)
            }
        }
    };
    Ok(tok_impl)
}

fn impl_fields(container: &TokenStream, fields: &Fields) -> Result<Vec<TokenStream>> {
    let res = match fields {
        Fields::Unit => vec![],
        Fields::Named(ref named) => named
            .named
            .iter()
            .enumerate()
            .map(|(idx, field)| impl_struct_field(idx, container, field))
            .try_fold(Vec::new(), |mut acc, item| {
                acc.push(item?);
                Result::Ok(acc)
            })?,
        Fields::Unnamed(ref unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(idx, field)| impl_struct_field(idx, container, field))
            .try_fold(Vec::new(), |mut acc, item| {
                acc.push(item?);
                Result::Ok(acc)
            })?,
    };
    Ok(res)
}

// Generate HasRefs for `struct foo { bar: Baz }`
pub(super) fn impl_struct(struct_: &ItemStruct) -> Result<TokenStream> {
    let container = Ident::new("self", Span::call_site()).to_token_stream();
    let mark_refs_impl = impl_fields(&container, &struct_.fields)?;
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

fn impl_enum_variant(variant: &Variant) -> Result<TokenStream> {
    let container = Ident::new("x", Span::call_site());
    let fields = impl_fields(&container.to_token_stream(), &variant.fields)?;
    let name = &variant.ident;
    let res = quote! {
        Self::#name(ref x) => {
           #( #fields )*;
        }
    };
    Ok(res)
}

fn impl_enum_contains_ref(enum_: &ItemEnum) -> Result<Vec<TokenStream>> {
    let mut types = HashSet::<Type>::new();
    for Variant { ref fields, .. } in enum_.variants.iter() {
        for field in fields.iter() {
            types.insert(field.ty.clone());
        }
    }

    let res = types
        .iter()
        .map(|ty| {
            quote! { <#ty>::contains_ref() }
        })
        .collect();
    Ok(res)
}

pub(super) fn impl_enum(enum_: &ItemEnum) -> Result<TokenStream> {
    let variants =
        enum_
            .variants
            .iter()
            .map(impl_enum_variant)
            .try_fold(Vec::new(), |mut acc, item| {
                acc.push(item?);
                Result::Ok(acc)
            })?;

    let contains_ref = impl_enum_contains_ref(enum_)?;
    let name = &enum_.ident;

    let res = quote! {
        impl rquickjs::HasRefs for #name {
            fn contains_ref() -> bool
                where Self: Sized
            {
                use rquickjs::HasRefs;
                false #(|| #contains_ref)*
            }

            fn mark_refs(&self, marker: &rquickjs::RefsMarker){
                match *self {
                    #(#variants)*
                }
            }
        }
    };
    Ok(res)
}
