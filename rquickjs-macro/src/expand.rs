mod function;
mod module;

use crate::{abort, util::crate_name, Tokens};
use quote::{format_ident, quote};
use syn::{Ident, Item};

pub struct Expander {
    /// rquickjs crate name
    pub lib_crate: Ident,

    /// register fn name ("register" by default)
    pub register_fn: Ident,
}

impl Expander {
    pub fn new() -> Self {
        let lib_crate = match crate_name("rquickjs") {
            Ok(name) => format_ident!("{}", name),
            Err(error) => abort!("Unable to determine rquickjs crate name ({})", error),
        };

        let register_fn = format_ident!("register");

        Self {
            lib_crate,
            register_fn,
        }
    }

    pub fn wrap_register_fn(&self, ident: Option<&Ident>, body: Tokens) -> Tokens {
        let lib_crate = &self.lib_crate;
        let ident = if let Some(ident) = ident {
            format_ident!("{}_{}", &self.register_fn, ident)
        } else {
            format_ident!("{}", &self.register_fn)
        };
        quote! {
            pub fn #ident<'js>(ctx: #lib_crate::Ctx<'js>, obj: #lib_crate::Object<'js>) -> #lib_crate::Result<()> {
                #body
            }
        }
    }

    /// Expand
    pub fn expand(&self, item: &Item) -> Tokens {
        let path = Vec::new();

        let binding = self
            .item(&path, item)
            .map(|(ident, body)| self.wrap_register_fn(ident, body));

        quote! {
            #item
            #binding
        }
    }

    /// Expand item
    pub fn item<'a>(
        &self,
        path: &Vec<&Ident>,
        item: &'a Item,
    ) -> Option<(Option<&'a Ident>, Tokens)> {
        Some(match item {
            Item::Fn(item) => (Some(&item.sig.ident), self.function(path, item)),
            Item::Mod(item) => (Some(&item.ident), self.module(path, item, false)),
            _ => return None,
        })
    }

    /// Expand path
    pub fn path(&self, path: &Vec<&Ident>, name: &Ident) -> Tokens {
        quote! {
            #(#path::)* #name
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::{format_ident, quote};

    #[test]
    fn initial_path() {
        let expander = Expander::new();
        let path = vec![];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn nested_path() {
        let expander = Expander::new();
        let seg1 = format_ident!("mod_a");
        let path = vec![&seg1];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            mod_a::name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn nested_nested_path() {
        let expander = Expander::new();
        let seg1 = format_ident!("mod_a");
        let seg2 = format_ident!("mod_b");
        let path = vec![&seg1, &seg2];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            mod_a::mod_b::name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
