use convert_case::Case as ConvertCase;
use darling::FromMeta;
use proc_macro2::{Ident, Span};
use proc_macro_crate::FoundCrate;
use proc_macro_error::abort_call_site;
use quote::format_ident;
use syn::{fold::Fold, Generics, Lifetime, LifetimeParam, Type};

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
#[derive(FromMeta, Clone, Copy, Eq, PartialEq, Debug)]
pub enum Case {
    #[darling(rename = "lowercase")]
    Lower,
    #[darling(rename = "UPPERCASE")]
    Upper,
    #[darling(rename = "camelCase")]
    Camel,
    #[darling(rename = "PascalCase")]
    Pascal,
    #[darling(rename = "snake_case")]
    Snake,
    #[darling(rename = "SCREAMING_SNAKE_CASE")]
    ScreamingSnake,
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

pub(crate) fn crate_ident() -> Ident {
    match proc_macro_crate::crate_name("rquickjs") {
        Err(e) => {
            abort_call_site!("could not find rquickjs package"; note = e);
        }
        Ok(FoundCrate::Itself) => {
            format_ident!("rquickjs")
        }
        Ok(FoundCrate::Name(x)) => {
            format_ident!("{}", x)
        }
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
