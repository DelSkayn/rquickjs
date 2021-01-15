use core::ops::Deref;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream, Result},
    token::Paren,
    Token,
};

/// A newtype for testing
///
/// This needed because AttributeArgs from syn crate is not a newtype and does not implements `Parse` trait
#[derive(Debug)]
pub struct AttributeArgs(pub syn::AttributeArgs);

impl Deref for AttributeArgs {
    type Target = syn::AttributeArgs;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut metas = Vec::new();

        loop {
            if input.is_empty() {
                break;
            }
            let value = input.parse()?;
            metas.push(value);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self(metas))
    }
}

#[derive(Debug)]
pub struct Parenthesized<T> {
    pub paren_token: Paren,
    pub content: T,
}

impl<T: Parse> Parse for Parenthesized<T> {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let paren_token = parenthesized!(content in input);
        let content = content.parse()?;
        Ok(Self {
            paren_token,
            content,
        })
    }
}
