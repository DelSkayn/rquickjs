use std::{env, path::Path};

use crate::common::crate_ident;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use rquickjs_core::{Context, Module, Result as JsResult, Runtime, WriteOptions};
use syn::{
    Error, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

/// A line of embedded modules.
pub struct EmbedModule {
    pub name: LitStr,
    pub path: Option<(Token![:], LitStr)>,
}

impl Parse for EmbedModule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<LitStr>()?;
        let path = if input.peek(Token![:]) {
            let colon = input.parse()?;
            let name = input.parse()?;
            Some((colon, name))
        } else {
            None
        };

        Ok(EmbedModule { path, name })
    }
}

/// The parsing struct for embedded modules.
pub struct EmbedModules(pub Punctuated<EmbedModule, Token![,]>);

impl Parse for EmbedModules {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let res = input.parse_terminated(EmbedModule::parse, Token![,])?;
        Ok(EmbedModules(res))
    }
}

/// Implementation of the macro
pub fn embed(modules: EmbedModules) -> Result<TokenStream> {
    let mut files = Vec::new();
    for f in modules.0.into_iter() {
        let path = f
            .path
            .as_ref()
            .map(|x| x.1.value())
            .unwrap_or_else(|| f.name.value());

        let path = Path::new(&path);

        let path = if path.is_relative() {
            let full_path = Path::new(
                &env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set"),
            )
            .join(path);
            match full_path.canonicalize() {
                Ok(x) => x,
                Err(e) => {
                    return Err(Error::new(
                        f.name.span(),
                        format_args!(
                            "Error loading embedded js module from path `{}`: {}",
                            full_path.display(),
                            e
                        ),
                    ));
                }
            }
        } else {
            path.to_owned()
        };

        let source = match std::fs::read_to_string(&path) {
            Ok(x) => x,
            Err(e) => {
                return Err(Error::new(
                    f.name.span(),
                    format_args!(
                        "Error loading embedded js module from path `{}`: {}",
                        path.display(),
                        e
                    ),
                ));
            }
        };
        files.push((f.name.value(), source));
    }

    let res = (|| -> JsResult<Vec<(String, Vec<u8>)>> {
        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;

        let mut modules = Vec::new();

        ctx.with(|ctx| -> JsResult<()> {
            for f in files.into_iter() {
                let bc = Module::declare(ctx.clone(), f.0.clone(), f.1)?
                    .write(WriteOptions::default())?;
                modules.push((f.0, bc));
            }
            Ok(())
        })?;
        Ok(modules)
    })();

    let res = match res {
        Ok(x) => x,
        Err(e) => {
            return Err(Error::new(
                Span::call_site(),
                format_args!("Error compiling embedded js module: {}", e),
            ));
        }
    };

    let res = to_entries(res.into_iter());

    expand(&res)
}

fn to_entries(modules: impl Iterator<Item = (String, Vec<u8>)>) -> Vec<(String, TokenStream)> {
    modules
        .map(|(name, data)| (name, quote! { &[#(#data),*] }))
        .collect::<Vec<_>>()
}

#[cfg(feature = "phf")]
pub fn expand(modules: &[(String, TokenStream)]) -> Result<TokenStream> {
    let keys = modules.iter().map(|(x, _)| x.clone()).collect::<Vec<_>>();

    let state = phf_generator::generate_hash(&keys);

    let key = state.key;
    let disps = state.disps.iter().map(|&(d1, d2)| quote!((#d1, #d2)));
    let entries = state.map.iter().map(|&idx| {
        let key = &modules[idx].0;
        let value = &modules[idx].1;
        quote!((#key, #value))
    });

    let lib_crate = crate_ident()?;
    let lib_crate = format_ident!("{}", lib_crate);
    Ok(quote! {
        #lib_crate::loader::bundle::Bundle(& #lib_crate::phf::Map{
            key: #key,
            disps: &[#(#disps),*],
            entries: &[#(#entries),*],
        })
    })
}

#[cfg(not(feature = "phf"))]
pub fn expand(modules: &[(String, TokenStream)]) -> Result<TokenStream> {
    let lib_crate = crate_ident()?;
    let lib_crate = format_ident!("{}", lib_crate);
    let entries = modules.iter().map(|(name, data)| {
        quote! { (#name,#data)}
    });
    Ok(quote! {
        #lib_crate::loader::bundle::Bundle(&[#(#entries),*])
    })
}

#[cfg(test)]
mod test {
    use super::{EmbedModules, expand, to_entries};
    use quote::quote;

    #[cfg(feature = "phf")]
    #[test]
    fn test_expand() {
        let data = vec![("test_module".to_string(), vec![1u8, 2, 3, 4])];
        let test_data = to_entries(data.into_iter());
        let tokens = expand(&test_data);
        let expected = quote! {
            rquickjs::loader::bundle::Bundle(&rquickjs::phf::Map{
                key: 12913932095322966823u64,
                disps: &[(0u32,0u32)],
                entries: &[
                    ("test_module", &[1u8, 2u8, 3u8,4u8])
                ],
            })
        };
        assert_eq_tokens!(tokens.unwrap(), expected);
    }

    #[cfg(not(feature = "phf"))]
    #[test]
    fn test_expand() {
        let data = vec![("test_module".to_string(), vec![1u8, 2, 3, 4])];
        let test_data = to_entries(data.into_iter());
        let tokens = expand(&test_data);
        let expected = quote! {
            rquickjs::loader::bundle::Bundle(&[
                ("test_module", &[1u8, 2u8, 3u8,4u8])
            ])
        };
        assert_eq_tokens!(tokens.unwrap(), expected);
    }

    #[test]
    fn parse() {
        let data = quote! {
            "Hello world": "foo",
            "bar"
        };
        let mods = syn::parse2::<EmbedModules>(data).unwrap();
        assert_eq!(mods.0.len(), 2);
        let mut iter = mods.0.iter();
        let a = iter.next().unwrap();
        assert_eq!(a.name.value(), "Hello world");
        assert_eq!(a.path.as_ref().unwrap().1.value(), "foo");
        let b = iter.next().unwrap();
        assert_eq!(b.name.value(), "bar");
        assert!(b.path.is_none());
        assert!(iter.next().is_none());
    }
}
