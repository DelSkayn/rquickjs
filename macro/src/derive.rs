#[cfg(test)]
macro_rules! test_cases {
    (
        $lib_crate_ident:ident,
        $($c:ident $k:ident { $($s:tt)* } { $($d:tt)* };)*
    ) => {
        $(
            #[test]
            fn $c() {
                let config = crate::Config::default();
                let $lib_crate_ident = &config.lib_crate;
                let input: syn::DeriveInput = syn::parse_quote! { $($s)* };
                let attrs: crate::DataType = darling::FromDeriveInput::from_derive_input(&input).unwrap();
                let binder = test_cases!(@macro_new $k)(attrs.config());
                let output = test_cases!(@expand binder attrs $k);
                let expected = quote::quote! { $($d)* };
                assert_eq_tokens!(output, expected);
            }
        )*
    };

    (@macro_new IntoJsByRef) => { crate::IntoJs::new };
    (@macro_new $k:ident) => { crate::$k::new };

    (@expand $binder:ident $attrs:ident IntoJsByRef) => { $binder.expand(&$attrs, true) };
    (@expand $binder:ident $attrs:ident IntoJs) => { $binder.expand(&$attrs, false) };
    (@expand $binder:ident $attrs:ident $k:ident) => { $binder.expand(&$attrs) };
}

mod attrs;
mod from_js;
mod has_refs;
mod into_js;

pub use attrs::*;
pub use from_js::*;
pub use has_refs::*;
pub use into_js::*;
