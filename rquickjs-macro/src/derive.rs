#[cfg(test)]
macro_rules! test_cases {
    ($($c:ident $k:ident { $($s:tt)* } { $($d:tt)* };)*) => {
        $(
            #[test]
            fn $c() {
                let input: syn::DeriveInput = syn::parse_quote! { $($s)* };
                let binder = crate::$k::new(crate::Config::new());
                let output = binder.expand(input);
                let expected = quote::quote! { $($d)* };
                assert_eq!(output.to_string(), expected.to_string());
            }
        )*
    };
}

mod from_js;
mod into_js;

pub use from_js::*;
pub use into_js::*;

use crate::Ident;
use proc_macro2::Span;
use syn::{punctuated::Punctuated, token::Comma, Fields, GenericParam, Lifetime};

fn new_lifetime(name: &str) -> Lifetime {
    Lifetime::new(name, Span::call_site())
}

fn has_lifetime(params: &Punctuated<GenericParam, Comma>, lifetime: &Lifetime) -> bool {
    params.iter().any(|param| {
        if let GenericParam::Lifetime(def) = param {
            &def.lifetime == lifetime
        } else {
            false
        }
    })
}

enum DataContent {
    Fields(Vec<Ident>),
    Points(usize),
    Nothing,
}

fn data_fields(fields: &Fields) -> DataContent {
    use Fields::*;
    match fields {
        Named(fields) => DataContent::Fields(
            fields
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap().clone())
                .collect(),
        ),
        Unnamed(fields) => DataContent::Points(fields.unnamed.len()),
        Unit => DataContent::Nothing,
    }
}
