use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Result};

use crate::{
    attrs::{take_attributes, OptionList},
    common::crate_ident,
    trace::{ImplConfig, TraceOption},
};

pub(crate) fn expand(mut input: DeriveInput) -> Result<TokenStream> {
    let name = input.ident;

    let mut config = ImplConfig::default();
    take_attributes(&mut input.attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<TraceOption> = attr.parse_args()?;
        options.0.iter().for_each(|x| config.apply(x));
        Ok(true)
    })?;

    let crate_name = if let Some(x) = config.crate_.clone() {
        format_ident!("{x}")
    } else {
        format_ident!("{}", crate_ident()?)
    };

    let res = quote! {
        impl<'js> #crate_name::JsLifetime<'js> for #name<'js>{
            type Changed<'to> = $name<'to>;
        }
    };
    Ok(res)
}
