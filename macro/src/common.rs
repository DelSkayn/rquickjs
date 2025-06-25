use convert_case::Case as ConvertCase;
use proc_macro_crate::FoundCrate;
use proc_macro2::Span;
use quote::{ToTokens, TokenStreamExt};
use syn::{
    Error, Generics, Lifetime, LifetimeParam, LitStr, Result, Type,
    fold::Fold,
    parse::{Parse, ParseStream},
};

/// prefix for getter implementations
pub const GET_PREFIX: &str = "__impl_get_";
/// prefix for setter implementations
pub const SET_PREFIX: &str = "__impl_set_";
/// the base prefix for type which should be accessed by macro users.
pub const BASE_PREFIX: &str = "js_";
/// the base prefix for type which should remain macro internal.
pub const IMPL_PREFIX: &str = "__impl_";

/// Casing for mass case convert.
///
/// Only allowing casings which are valid js identifiers.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Case {
    Lower,
    Upper,
    Camel,
    Pascal,
    Snake,
    ScreamingSnake,
}

impl Parse for Case {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let str_lit: LitStr = input.parse()?;
        let value = str_lit.value();
        match value.as_str() {
            "lowercase" => Ok(Case::Lower),
            "UPPERCASE" => Ok(Case::Upper),
            "camelCase" => Ok(Case::Camel),
            "PascalCase" => Ok(Case::Pascal),
            "snake_case" => Ok(Case::Snake),
            "SCREAMING_SNAKE" => Ok(Case::ScreamingSnake),
            _ => Err(syn::Error::new(
                str_lit.span(),
                "Invalid casing, expected one of 'lowercase', 'UPPERCASE', 'camelCase','PascalCase','snake_case','SCREAMING_SNAKE'",
            )),
        }
    }
}

impl ToTokens for Case {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Case::Lower => tokens.append_all(["lowercase"]),
            Case::Upper => tokens.append_all(["UPPERCASE"]),
            Case::Camel => tokens.append_all(["camelCase"]),
            Case::Pascal => tokens.append_all(["PascalCase"]),
            Case::Snake => tokens.append_all(["snake_case"]),
            Case::ScreamingSnake => tokens.append_all(["SCREAMING_SNAKE"]),
        }
    }
}

impl Case {
    pub fn to_convert_case(self) -> ConvertCase {
        match self {
            Case::Lower => ConvertCase::Lower,
            Case::Upper => ConvertCase::Upper,
            Case::Camel => ConvertCase::Camel,
            Case::Pascal => ConvertCase::Pascal,
            Case::Snake => ConvertCase::Snake,
            Case::ScreamingSnake => ConvertCase::ScreamingSnake,
        }
    }
}

pub(crate) fn crate_ident() -> Result<String> {
    match proc_macro_crate::crate_name("rquickjs") {
        Err(e) => Err(Error::new(
            Span::call_site(),
            format_args!("could not find rquickjs package: {e}"),
        )),
        Ok(FoundCrate::Itself) => Ok("rquickjs".to_owned()),
        Ok(FoundCrate::Name(x)) => Ok(x.to_string()),
    }
}

/// Add the 'js lifetime to a list of existing lifetimes, if it doesn't already exits.
pub fn add_js_lifetime(generics: &Generics) -> Generics {
    let mut generics = generics.clone();
    let has_js_lifetime = generics.lifetimes().any(|lt| lt.lifetime.ident == "js");
    if !has_js_lifetime {
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(LifetimeParam::new(Lifetime::new(
                "'js",
                Span::call_site(),
            ))),
        );
    }
    generics
}

pub struct SelfReplacer<'a> {
    pub ty: &'a Type,
}

impl<'a> SelfReplacer<'a> {
    pub fn with(ty: &'a Type) -> Self {
        Self { ty }
    }
}

impl<'a> Fold for SelfReplacer<'a> {
    fn fold_type(&mut self, i: Type) -> Type {
        let Type::Path(x) = i else { return i };
        if x.path.segments.len() != 1 {
            return Type::Path(self.fold_type_path(x));
        }
        if x.path.segments.first().unwrap().ident == "Self" {
            self.ty.clone()
        } else {
            Type::Path(self.fold_type_path(x))
        }
    }
}

pub(crate) mod kw {
    syn::custom_keyword!(frozen);
    syn::custom_keyword!(skip_trace);
    syn::custom_keyword!(rename);
    syn::custom_keyword!(rename_all);
    syn::custom_keyword!(rename_vars);
    syn::custom_keyword!(rename_types);
    syn::custom_keyword!(get);
    syn::custom_keyword!(set);
    syn::custom_keyword!(constructor);
    syn::custom_keyword!(skip);
    syn::custom_keyword!(configurable);
    syn::custom_keyword!(enumerable);
    syn::custom_keyword!(prefix);
    syn::custom_keyword!(declare);
    syn::custom_keyword!(evaluate);
}
