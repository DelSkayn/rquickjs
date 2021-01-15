use crate::TokenStream;
use darling::{util::Override, FromMeta};
use quote::{quote, ToTokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromMeta)]
pub enum PubVis {
    #[darling(rename = "self")]
    Self_,
    #[darling(rename = "super")]
    Super,
    #[darling(rename = "crate")]
    Crate,
}

impl ToTokens for PubVis {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use PubVis::*;
        match self {
            Self_ => quote!(self),
            Super => quote!(super),
            Crate => quote!(crate),
        }
        .to_tokens(tokens)
    }
}

impl PubVis {
    pub fn override_tokens(this: &Override<PubVis>) -> TokenStream {
        match this {
            Override::Inherit => quote!(pub),
            Override::Explicit(vis) => quote!(pub(#vis)),
        }
    }
}
