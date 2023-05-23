use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rquickjs_core::{Context, Module, Result, Runtime};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    LitStr, Token,
};

/// A line of embedded modules.
pub struct EmbedModule {
    pub path: LitStr,
    pub name: Option<(Token![:], LitStr)>,
}

impl Parse for EmbedModule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse::<LitStr>()?;
        let name = if input.peek(Token![:]) {
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
        let res = input.parse_terminated::<_, Token![,]>(EmbedModule::parse)?;
        Ok(EmbedModules(res))
    }
}

/// Implementation of the macro
pub fn embed(modules: EmbedModules) -> TokenStream {
    let mut files = Vec::new();
    for f in modules.0.into_iter() {
        let path = f.path.value();
        let source = match std::fs::read_to_string(&path) {
            Ok(x) => x,
            Err(e) => {
                error!(
                    f.path,
                    "Error loading embedded js module from path `{}`: {}", path, e
                );
                continue;
            }
        };
        files.push((f.name.map(|x| x.1.value()).unwrap_or(path), source));
    }

    let res = (|| -> Result<Vec<(String, Vec<u8>)>> {
        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;

        let mut modules = Vec::new();

        ctx.with(|ctx| -> Result<()> {
            for f in files.into_iter() {
                let bc =
                    unsafe { Module::unsafe_declare(ctx, f.0.clone(), f.1)?.write_object(false)? };
                modules.push((f.0, bc));
            }
            Ok(())
        })?;
        Ok(modules)
    })();

    let res = match res {
        Ok(x) => x,
        Err(e) => {
            error!("Error compiling embedded js module: {}", e);
            return quote!();
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
pub fn expand(modules: &[(String, TokenStream)]) -> TokenStream {
    let keys = modules.iter().map(|(x, _)| x.clone()).collect::<Vec<_>>();

    let state = phf_generator::generate_hash(&keys);

    let key = state.key;
    let disps = state.disps.iter().map(|&(d1, d2)| quote!((#d1, #d2)));
    let entries = state.map.iter().map(|&idx| {
        let key = &modules[idx].0;
        let value = &modules[idx].1;
        quote!((#key, #value))
    });

    let lib_crate = super::config::lib_crate();
    let lib_crate = format_ident!("{}", lib_crate);
    quote! {
        #lib_crate::loader::bundle::Bundle(& #lib_crate::phf::Map{
            key: #key,
            disps: &[#(#disps),*],
            entries: &[#(#entries),*],
        })
    }
}

#[cfg(not(feature = "phf"))]
pub fn expand(modules: &[(String, TokenStream)]) -> TokenStream {
    let lib_crate = super::config::lib_crate();
    let lib_crate = format_ident!("{}", lib_crate);
    let entries = modules.iter().map(|(name, data)| {
        quote! { (#name,#data)}
    });
    quote! {
        #lib_crate::loader::bundle::Bundle(&[#(#entries),*])
    }
}

#[cfg(test)]
mod test {
    use super::{expand, to_entries, EmbedModules};
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
        assert_eq_tokens!(tokens, expected);
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
        assert_eq_tokens!(tokens, expected);
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
        assert_eq!(a.path.value(), "Hello world");
        assert_eq!(a.name.as_ref().unwrap().1.value(), "foo");
        let b = iter.next().unwrap();
        assert_eq!(b.path.value(), "bar");
        assert!(b.name.is_none());
        assert!(iter.next().is_none());
    }
}
