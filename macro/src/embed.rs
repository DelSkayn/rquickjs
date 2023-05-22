#[cfg(test)]
macro_rules! test_cases {
    ($($(#[$m:meta])* $c:ident { $($a:tt)* } { $($s:tt)* } { $($d:tt)* };)*) => {
        $(
            $(#[$m])*
            #[test]
            fn $c() {
                let embedder = crate::Embedder::new(crate::Config::default());
                let attrs: crate::AttributeArgs = syn::parse_quote! { $($a)* };
                let attrs = darling::FromMeta::from_list(&*attrs).unwrap();
                let input = syn::parse_quote! { $($s)* };
                let output = embedder.expand(attrs, input);
                let actual = quote::quote! { #output };
                let expected = quote::quote! { $($d)* };
                assert_eq_tokens!(actual, expected);
            }
        )*
    };
}

mod attrs;
pub use attrs::*;

mod loader;

use crate::{Config, PubVis, TokenStream};
use ident_case::RenameRule;
use quote::{format_ident, quote};
use rquickjs_core::{
    loader::{FileResolver, ScriptLoader},
    Context, Module, Result, Runtime,
};
use std::path::Path;
use syn::ItemMod;

#[cfg(feature = "phf")]
use {phf_shared::PhfHash, std::hash::Hasher};

pub struct Entry<N, D> {
    name: N,
    data: D,
}

#[cfg(feature = "phf")]
impl<N, D> PhfHash for Entry<N, D>
where
    N: PhfHash,
{
    fn phf_hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.name.phf_hash(state)
    }
}

pub struct Embedder {
    config: Config,
}

impl Embedder {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn expand(&self, attrs: AttrEmbed, item: ItemMod) -> TokenStream {
        #[cfg(feature = "phf")]
        let phf_map = attrs.phf_map;

        let AttrEmbed {
            ident,
            paths,
            patterns,
            mut names,
            test,
            public,
            ..
        } = attrs;

        let ident = ident.unwrap_or_else(|| {
            format_ident!(
                "{}",
                RenameRule::ScreamingSnakeCase.apply_to_variant(item.ident.to_string())
            )
        });
        if names.is_empty() {
            names.push(item.ident.to_string());
        }
        let public = public.as_ref().map(PubVis::override_tokens);

        let compile = loader::Embed::new();

        let mut resolver = compile.resolver(FileResolver::default());
        for path in &paths {
            resolver.add_path(path);
        }
        for pattern in &patterns {
            resolver.add_pattern(pattern);
        }

        let mut loader = compile.loader(ScriptLoader::default());

        for pattern in &patterns {
            if let Some(extension) = Path::new(pattern)
                .extension()
                .and_then(|extension| extension.to_str())
            {
                loader.add_extension(extension);
            }
        }

        if let Err(error) = (|| -> Result<()> {
            let rt = Runtime::new()?;
            let ctx = Context::full(&rt)?;

            rt.set_loader(resolver, loader);

            let source = names
                .iter()
                .map(|name| format!("import '{name}';"))
                .collect::<Vec<_>>()
                .join("");

            ctx.with(|ctx| {
                Module::declare(ctx, "<main>", source)?;
                Ok(())
            })
        })() {
            error!(ident, "Error when embedding JS modules: {}", error);
            return quote!();
        }

        let entries = compile
            .bytecodes()
            .into_iter()
            .map(|(name, data)| {
                let name = name.to_string();
                let data = if test {
                    quote! { &[0u8, 1u8, 2u8, 3u8] }
                } else {
                    quote! { &[#(#data),*] }
                };
                Entry { name, data }
            })
            .collect::<Vec<_>>();

        let content = {
            #[cfg(feature = "phf")]
            if phf_map {
                self.build_phf(&entries)
            } else {
                self.build_sca(&entries)
            }

            #[cfg(not(feature = "phf"))]
            self.build_sca(&entries)
        };

        quote! {
            #public static #ident #content;
        }
    }

    fn build_sca(&self, entries: &[Entry<String, TokenStream>]) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let entries = entries.iter().map(|Entry { name, data }| {
            quote! { (#name, #data) }
        });
        quote! {
            : #lib_crate::loader::Bundle<&'static [(&'static str, &'static [u8])]> = #lib_crate::loader::Bundle(&[#(#entries),*])
        }
    }

    #[cfg(feature = "phf")]
    fn build_phf(&self, entries: &[Entry<String, TokenStream>]) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let state = phf_generator::generate_hash(entries);
        let key = state.key;
        let disps = state.disps.iter().map(|&(d1, d2)| quote!((#d1, #d2)));
        let entries = state.map.iter().map(|&index| {
            let Entry { name, data } = &entries[index];
            quote! { (#name, #data) }
        });
        quote! {
            : #lib_crate::loader::Bundle<&'static #lib_crate::phf::Map<&'static str, &'static [u8]>> = #lib_crate::loader::Bundle(&#lib_crate::phf::Map {
                key: #key,
                disps: #lib_crate::phf::Slice::Static(&[#(#disps),*]),
                entries: #lib_crate::phf::Slice::Static(&[#(#entries),*]),
            })
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        static_const_array { test, path = "." } { mod my_module {} } {
            static MY_MODULE: rquickjs::loader::Bundle<&'static [(&'static str, &'static [u8])]> = rquickjs::loader::Bundle(&[
                ("my_module", &[0u8, 1u8, 2u8, 3u8])
            ]);
        };

        #[cfg(feature = "phf")]
        perfect_hash_map { test, perfect, path = "." } { mod my_module {} } {
            static MY_MODULE: rquickjs::loader::Bundle<&'static rquickjs::phf::Map<&'static str, &'static [u8]>> = rquickjs::loader::Bundle(&rquickjs::phf::Map {
                key: 12913932095322966823u64,
                disps: rquickjs::phf::Slice::Static(&[
                    (0u32 , 0u32)
                ]),
                entries: rquickjs::phf::Slice::Static(&[
                    ("my_module", &[0u8, 1u8, 2u8, 3u8])
                ]),
            });
        };
    }
}
