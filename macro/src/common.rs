use convert_case::Case as ConvertCase;
use darling::FromMeta;
use proc_macro2::Ident;
use proc_macro_crate::FoundCrate;
use proc_macro_error::abort_call_site;
use quote::format_ident;

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
