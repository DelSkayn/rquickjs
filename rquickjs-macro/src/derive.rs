#[cfg(test)]
macro_rules! test_cases {
    ($($c:ident $k:ident { $($s:tt)* } { $($d:tt)* };)*) => {
        $(
            #[test]
            fn $c() {
                let input: syn::DeriveInput = syn::parse_quote! { $($s)* };
                let attrs: crate::DataType = darling::FromDeriveInput::from_derive_input(&input).unwrap();
                let binder = crate::$k::new(attrs.config());
                let output = binder.expand(&attrs);
                let expected = quote::quote! { $($d)* };
                assert_eq!(output.to_string(), expected.to_string());
            }
        )*
    };
}

mod attrs;
mod from_js;
mod into_js;

pub use attrs::*;
pub use from_js::*;
pub use into_js::*;
