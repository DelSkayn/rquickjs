use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Error, Result};

use crate::common::{Case, GET_PREFIX, SET_PREFIX};

use super::method::{Attributes, Method};

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
            check_getter_setter_attributes(&method, set)?;
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
            check_getter_setter_attributes(get, &method)?;
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

    /// The attributes of the accessor property. A getter and setter describe
    /// one property, so an attribute either of them sets applies to both.
    fn attributes(&self) -> Attributes {
        let get = self.get.as_ref().map(|g| &g.config);
        let set = self.set.as_ref().map(|s| &s.config);
        let attribute = |f: fn(&super::method::MethodConfig) -> Option<bool>| {
            get.and_then(f).or_else(|| set.and_then(f)).unwrap_or(false)
        };
        Attributes {
            configurable: attribute(|c| c.configurable),
            enumerable: attribute(|c| c.enumerable),
            writable: false,
        }
    }

    pub fn expand_apply_to(
        &self,
        lib_crate: &Ident,
        object_name: &Ident,
        case: Option<Case>,
    ) -> TokenStream {
        let attributes = self.attributes().expand();
        let (name, accessor) = match (self.get.as_ref(), self.set.as_ref()) {
            (Some(get), Some(set)) => {
                let get_name = get.function.expand_carry_type_name(GET_PREFIX);
                let set_name = set.function.expand_carry_type_name(SET_PREFIX);
                (
                    get.name(case),
                    quote!(#lib_crate::object::Accessor::new(#get_name, #set_name)),
                )
            }
            (Some(get), None) => {
                let get_name = get.function.expand_carry_type_name(GET_PREFIX);
                (
                    get.name(case),
                    quote!(#lib_crate::object::Accessor::new_get(#get_name)),
                )
            }
            (None, Some(set)) => {
                let set_name = set.function.expand_carry_type_name(SET_PREFIX);
                (
                    set.name(case),
                    quote!(#lib_crate::object::Accessor::new_set(#set_name)),
                )
            }
            (None, None) => return TokenStream::new(),
        };
        quote! {#object_name.prop(#name, #accessor #attributes)?;}
    }

    pub fn expand_apply_to_proto(&self, lib_crate: &Ident, case: Option<Case>) -> TokenStream {
        let proto = Ident::new("_proto", proc_macro2::Span::call_site());
        self.expand_apply_to(lib_crate, &proto, case)
    }
}

/// Reject conflicting attributes on the getter and setter for the same property
fn check_getter_setter_attributes(get: &Method, set: &Method) -> Result<()> {
    for (attribute, disagree) in [
        ("static", get.config.r#static != set.config.r#static),
        (
            "configurable",
            matches!((get.config.configurable, set.config.configurable), (Some(a), Some(b)) if a != b),
        ),
        (
            "enumerable",
            matches!((get.config.enumerable, set.config.enumerable), (Some(a), Some(b)) if a != b),
        ),
    ] {
        if disagree {
            let mut error = Error::new(
                set.attr_span,
                format_args!(
                    "getter and setter for the same property must agree on `{attribute}`."
                ),
            );
            error.combine(Error::new(get.attr_span, "getter defined here."));
            return Err(error);
        }
    }
    Ok(())
}
