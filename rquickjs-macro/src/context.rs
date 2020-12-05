use crate::{Ident, TokenStream};
use quote::{quote, ToTokens};
use std::fmt::{Display, Formatter, Result as FmtResult};
use syn::{PathArguments, PathSegment};

/// The source for import from
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source(Vec<PathSegment>);

impl Source {
    pub fn with_ident(&self, ident: Ident) -> Self {
        let mut path = self.0.clone();
        let ident = ident;
        path.push(PathSegment {
            ident,
            arguments: PathArguments::None,
        });
        Self(path)
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let src = self;
        quote!(#src).to_string().fmt(f)
    }
}

impl Default for Source {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl ToTokens for Source {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let segments = &self.0;
        quote!(#(#segments)::*).to_tokens(tokens)
    }
}
