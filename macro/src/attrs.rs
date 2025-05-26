use syn::{
    Attribute, LitBool, Token,
    parse::{Parse, ParseStream},
};

/// An value option with an assigned value.
#[derive(Debug)]
pub(crate) struct ValueOption<K, V> {
    pub _key: K,
    pub _assign: syn::token::Eq,
    pub value: V,
}

impl<K: Parse, V: Parse> Parse for ValueOption<K, V> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: K = input.parse()?;
        let assign: Token![=] = input.parse()?;
        let value: V = input.parse()?;

        Ok(ValueOption {
            _key: key,
            _assign: assign,
            value,
        })
    }
}

/// An flag option with an optionally assigned boolean value.
#[derive(Debug)]
pub(crate) struct FlagOption<K> {
    pub _key: K,
    pub value: Option<(syn::token::Eq, LitBool)>,
}

impl<K> FlagOption<K> {
    pub fn is_true(&self) -> bool {
        self.value.as_ref().map(|x| x.1.value).unwrap_or(true)
    }
}

impl<K: Parse> Parse for FlagOption<K> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: K = input.parse()?;
        let value = input
            .peek(Token![=])
            .then(|| {
                let assign: Token![=] = input.parse()?;
                let value: LitBool = input.parse()?;
                syn::Result::Ok((assign, value))
            })
            .transpose()?;

        Ok(FlagOption { _key: key, value })
    }
}

#[derive(Debug)]
pub struct OptionList<O>(pub Vec<O>);

impl<O: Parse> Parse for OptionList<O> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(OptionList(Vec::new()));
        }

        let mut res = Vec::new();
        loop {
            res.push(O::parse(input)?);
            if input.is_empty() {
                return Ok(OptionList(res));
            }
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                return Ok(OptionList(res));
            }
        }
    }
}

pub fn take_attributes(
    attrs: &mut Vec<Attribute>,
    mut extractor: impl FnMut(&Attribute) -> syn::Result<bool>,
) -> syn::Result<()> {
    *attrs = attrs
        .drain(..)
        .filter_map(|attr| {
            extractor(&attr)
                .map(|handled| if handled { None } else { Some(attr) })
                .transpose()
        })
        .collect::<syn::Result<_>>()?;

    Ok(())
}
