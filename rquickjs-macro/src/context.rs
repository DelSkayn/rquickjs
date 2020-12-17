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
        path.push(PathSegment {
            ident,
            arguments: PathArguments::None,
        });
        Self(path)
    }

    pub fn parent(&self) -> Self {
        let mut path = self.0.clone();
        path.pop();
        Self(path)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let mut iter = self.0.iter();
        if let Some(segment) = iter.next() {
            quote!(#segment).to_string().fmt(f)?;
            for segment in iter {
                ".".fmt(f)?;
                quote!(#segment).to_string().fmt(f)?;
            }
        }
        Ok(())
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
