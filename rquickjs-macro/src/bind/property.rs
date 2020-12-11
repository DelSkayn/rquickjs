use super::{BindConst, BindFn};
use crate::{abort, Config, TokenStream};
use quote::quote;
use syn::spanned::Spanned;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindProp {
    pub val: Option<BindConst>,
    pub get: Option<BindFn>,
    pub set: Option<BindFn>,
}

macro_rules! prevent_overriding {
    ($this:ident $span:ident $name:ident: $($field:ident $entity:ident,)*) => {
        $(
            if let Some(ent) = &$this.$field {
                abort!(
                    $span.span(),
                    concat!("Property `{}` already defined with ", stringify!($entity), " `{}`"),
                    $name,
                    ent.src
                );
            }
        )*
    };
}

impl BindProp {
    pub fn set_const<S: Spanned>(&mut self, span: S, name: &str, val: BindConst) {
        prevent_overriding! {
            self span name:
            val const,
            get getter,
            set setter,
        }
        self.val = Some(val);
    }

    pub fn is_static(&self) -> bool {
        self.val.is_some()
            || match (&self.get, &self.set) {
                (Some(get), Some(set)) => !get.method && !set.method,
                (Some(get), _) => !get.method,
                _ => true,
            }
    }

    pub fn set_getter<S: Spanned>(&mut self, span: S, name: &str, get: BindFn) {
        prevent_overriding! {
            self span name:
            val const,
            get getter,
        }
        self.get = Some(get);
    }

    pub fn set_setter<S: Spanned>(&mut self, span: S, name: &str, set: BindFn) {
        prevent_overriding! {
            self span name:
            val const,
            set setter,
        }
        self.set = Some(set);
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let exports_var = &cfg.exports_var;

        let value = match (&self.get, &self.set, &self.val) {
            (Some(get), Some(set), _) => {
                let get = get.expand_pure(cfg);
                let set = set.expand_pure(cfg);
                quote! { (#get, #set) }
            }
            (Some(get), _, _) => {
                let get = get.expand_pure(cfg);
                quote! { (#get, ) }
            }
            (_, _, Some(val)) => {
                let val = val.expand_pure(cfg);
                quote! { (#val, ) }
            }
            _ => {
                abort!("{}", "Misconfigured property");
            }
        };
        quote! { #exports_var.prop(#name, #value)?; }
    }
}
