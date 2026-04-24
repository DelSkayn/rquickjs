use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Error, Result};

use crate::common::{Case, GET_PREFIX, SET_PREFIX};

use super::method::Method;

pub struct JsAccessor {
    get: Option<Method>,
    set: Option<Method>,
}

impl JsAccessor {
    pub fn new() -> Self {
        JsAccessor {
            get: None,
            set: None,
        }
    }

    pub(crate) fn define_get(&mut self, method: Method, rename: Option<Case>) -> Result<()> {
        if let Some(first_getter) = self.get.as_ref() {
            let first_span = first_getter.attr_span;

            let mut error = Error::new(
                method.attr_span,
                format_args!("Redefined a getter for `{:?}`.", method.name(rename)),
            );
            error.combine(Error::new(first_span, "Getter first defined here."));
            return Err(error);
        }
        if let Some(set) = self.set.as_ref() {
            if set.config.r#static != method.config.r#static {
                let mut error = Error::new(
                    method.attr_span,
                    "getter and setter for the same property must agree on `static`.",
                );
                error.combine(Error::new(set.attr_span, "setter defined here."));
                return Err(error);
            }
        }
        self.get = Some(method);
        Ok(())
    }

    pub(crate) fn define_set(&mut self, method: Method, rename: Option<Case>) -> Result<()> {
        if let Some(first_setter) = self.set.as_ref() {
            let first_span = first_setter.attr_span;
            let mut error = Error::new(
                method.attr_span,
                format_args!("Redefined a setter for `{:?}`.", method.name(rename)),
            );
            error.combine(Error::new(first_span, "Setter first defined here."));
            return Err(error);
        }
        if let Some(get) = self.get.as_ref() {
            if get.config.r#static != method.config.r#static {
                let mut error = Error::new(
                    method.attr_span,
                    "getter and setter for the same property must agree on `static`.",
                );
                error.combine(Error::new(get.attr_span, "getter defined here."));
                return Err(error);
            }
        }
        self.set = Some(method);
        Ok(())
    }

    pub fn is_static(&self) -> bool {
        self.get
            .as_ref()
            .map(|g| g.config.r#static)
            .or_else(|| self.set.as_ref().map(|s| s.config.r#static))
            .unwrap_or(false)
    }

    pub fn expand_impl(&self) -> TokenStream {
        let mut res = TokenStream::new();
        if let Some(ref x) = self.get {
            res.extend(x.expand_impl());
        }
        if let Some(ref x) = self.set {
            res.extend(x.expand_impl());
        }
        res
    }

    pub fn expand_js_impl(&self, lib_crate: &Ident) -> TokenStream {
        let mut res = TokenStream::new();
        if let Some(ref g) = self.get {
            res.extend(g.expand_js_impl(GET_PREFIX, lib_crate));
        }
        if let Some(ref s) = self.set {
            res.extend(s.expand_js_impl(SET_PREFIX, lib_crate));
        }
        res
    }

    pub fn expand_apply_to(
        &self,
        lib_crate: &Ident,
        object_name: &Ident,
        case: Option<Case>,
    ) -> TokenStream {
        match (self.get.as_ref(), self.set.as_ref()) {
            (Some(get), Some(set)) => {
                let configurable = get.config.configurable || set.config.configurable;
                let enumerable = get.config.enumerable || set.config.enumerable;

                let name = get.name(case);

                let configurable = if configurable {
                    quote!(.configurable())
                } else {
                    Default::default()
                };
                let enumerable = if enumerable {
                    quote!(.enumerable())
                } else {
                    Default::default()
                };
                let get_name = get.function.expand_carry_type_name(GET_PREFIX);
                let set_name = set.function.expand_carry_type_name(SET_PREFIX);
                quote! {#object_name.prop(#name,
                        #lib_crate::object::Accessor::new(#get_name,#set_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (Some(get), None) => {
                let configurable = get.config.configurable;
                let enumerable = get.config.enumerable;

                let name = get.name(case);

                let configurable = if configurable {
                    quote!(.configurable())
                } else {
                    Default::default()
                };
                let enumerable = if enumerable {
                    quote!(.enumerable())
                } else {
                    Default::default()
                };
                let get_name = get.function.expand_carry_type_name(GET_PREFIX);
                quote! {#object_name.prop(#name,
                        #lib_crate::object::Accessor::new_get(#get_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (None, Some(set)) => {
                let configurable = set.config.configurable;
                let enumerable = set.config.enumerable;

                let name = set.name(case);

                let configurable = if configurable {
                    quote!(.configurable())
                } else {
                    Default::default()
                };
                let enumerable = if enumerable {
                    quote!(.enumerable())
                } else {
                    Default::default()
                };

                let set_name = set.function.expand_carry_type_name(GET_PREFIX);
                quote! {#object_name.prop(#name,
                        #lib_crate::object::Accessor::new_set(#set_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (None, None) => TokenStream::new(),
        }
    }

    pub fn expand_apply_to_proto(&self, lib_crate: &Ident, case: Option<Case>) -> TokenStream {
        let proto = Ident::new("_proto", proc_macro2::Span::call_site());
        self.expand_apply_to(lib_crate, &proto, case)
    }
}
