use super::{function::BindFn1, BindConst};
use crate::{config::Config, TokenStream};
use quote::quote;
use syn::spanned::Spanned;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindProp {
    pub val: Option<BindConst>,
    pub get: Option<BindFn1>,
    pub set: Option<BindFn1>,
    pub writable: bool,
    pub configurable: bool,
    pub enumerable: bool,
}

macro_rules! prevent_overriding {
    ($this:ident $span:ident $name:ident: $($field:ident $entity:ident,)*) => {
        $(
            if let Some(ent) = &$this.$field {
                error!(
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

    pub fn set_getter<S: Spanned>(&mut self, span: S, name: &str, get: BindFn1) {
        prevent_overriding! {
            self span name:
            val const,
            get getter,
        }
        self.get = Some(get);
    }

    pub fn set_setter<S: Spanned>(&mut self, span: S, name: &str, set: BindFn1) {
        prevent_overriding! {
            self span name:
            val const,
            set setter,
        }
        self.set = Some(set);
    }

    pub fn set_writable<S: Spanned>(&mut self, span: S, flag: bool) {
        if flag {
            if self.get.is_some() || self.set.is_some() {
                warning!(
                    span.span(),
                    "The property defined with getter and/or setter cannot has `writable` flag"
                );
            } else {
                self.writable = true;
            }
        }
    }

    pub fn set_configurable(&mut self, flag: bool) {
        if flag {
            self.configurable = true;
        }
    }

    pub fn set_enumerable(&mut self, flag: bool) {
        if flag {
            self.enumerable = true;
        }
    }

    pub fn expand(&self, name: &str, cfg: &Config, is_module: bool) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;

        if is_module {
            error!("A module can't export properties, for prop: '{}'", name);
            return quote! {};
        }

        let mut value = match (&self.get, &self.set, &self.val) {
            (Some(get), Some(set), _) => {
                let get = get.expand_pure(cfg);
                let set = set.expand_pure(cfg);
                quote! { #lib_crate::object::Accessor::new(#get, #set) }
            }
            (Some(get), _, _) => {
                let get = get.expand_pure(cfg);
                quote! { #lib_crate::object::Accessor::new_get(#get) }
            }
            (_, Some(set), _) => {
                let set = set.expand_pure(cfg);
                quote! { #lib_crate::object::Accessor::new_set(#set) }
            }
            (_, _, Some(val)) => {
                let val = val.expand_pure(cfg);
                quote! { #lib_crate::object::Property::from(#val) }
            }
            _ => {
                error!("Misconfigured property '{}'", name);
                quote! {}
            }
        };
        if self.writable {
            value.extend(quote! { .writable() });
        }
        if self.configurable {
            value.extend(quote! { .configurable() });
        }
        if self.enumerable {
            value.extend(quote! { .enumerable() });
        }
        quote! { #exports_var.prop(#name, #value)?; }
    }
}
