use convert_case::Casing;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Attribute, Error, Ident, LitStr, Path, Result, Token,
    parse::{Parse, ParseStream},
    parse_quote,
};

use crate::{
    attrs::{FlagOption, ValueOption},
    class::{ClassConfig, ClassOption},
    common::{Case, crate_ident, kw},
    function::{FunctionConfig, FunctionOption},
};

#[derive(Debug, Default)]
pub(crate) struct ModuleConfig {
    pub crate_: Option<String>,
    pub prefix: Option<String>,
    pub rename: Option<String>,
    pub rename_vars: Option<Case>,
    pub rename_types: Option<Case>,
}

impl ModuleConfig {
    pub fn apply(&mut self, option: &ModuleOption) {
        match option {
            ModuleOption::Crate(x) => {
                self.crate_ = Some(x.value.value());
            }
            ModuleOption::RenameVars(x) => {
                self.rename_vars = Some(x.value);
            }
            ModuleOption::RenameTypes(x) => {
                self.rename_types = Some(x.value);
            }
            ModuleOption::Rename(x) => {
                self.rename = Some(x.value.value());
            }
            ModuleOption::Prefix(x) => {
                self.prefix = Some(x.value.value());
            }
        }
    }

    pub fn crate_name(&self) -> Result<String> {
        self.crate_.clone().map(Ok).unwrap_or_else(crate_ident)
    }

    pub fn carry_name(&self, name: &Ident) -> Ident {
        Ident::new(
            &format!("{}{}", self.prefix.as_deref().unwrap_or("js_"), name),
            name.span(),
        )
    }
}

pub(crate) enum ModuleOption {
    Prefix(ValueOption<kw::prefix, LitStr>),
    Crate(ValueOption<Token![crate], LitStr>),
    RenameVars(ValueOption<kw::rename_vars, Case>),
    RenameTypes(ValueOption<kw::rename_types, Case>),
    Rename(ValueOption<kw::rename, LitStr>),
}

impl Parse for ModuleOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::prefix) {
            input.parse().map(Self::Prefix)
        } else if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else if input.peek(kw::rename_vars) {
            input.parse().map(Self::RenameVars)
        } else if input.peek(kw::rename_types) {
            input.parse().map(Self::RenameTypes)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else {
            Err(syn::Error::new(input.span(), "invalid module attribute"))
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct ModuleFunctionConfig {
    pub declare: bool,
    pub evaluate: bool,
    pub skip: bool,
    pub function: FunctionConfig,
}

impl ModuleFunctionConfig {
    pub fn apply(&mut self, option: &ModuleFunctionOption) {
        match option {
            ModuleFunctionOption::Declare(x) => {
                self.declare = x.is_true();
            }
            ModuleFunctionOption::Evaluate(x) => {
                self.evaluate = x.is_true();
            }
            ModuleFunctionOption::Function(x) => {
                self.function.apply(x);
            }
            ModuleFunctionOption::Skip(x) => {
                self.skip = x.is_true();
            }
        }
    }

    pub fn reexpand(&self) -> Option<Attribute> {
        let mut attrs = Vec::new();
        if let Some(x) = self.function.crate_.as_deref() {
            attrs.push(quote!(crate = #x));
        }
        if let Some(x) = self.function.prefix.as_deref() {
            attrs.push(quote!(prefix = #x));
        }
        if let Some(x) = self.function.rename.as_deref() {
            attrs.push(quote!(rename = #x));
        }
        if attrs.is_empty() {
            None
        } else {
            Some(parse_quote! {
                #[qjs(#(#attrs),*)]
            })
        }
    }

    pub fn validate(&self, span: Span) -> Result<()> {
        if self.skip && self.evaluate {
            return Err(Error::new(span, "Can't skip the module evaluate function"));
        }
        if self.skip && self.declare {
            return Err(Error::new(span, "Can't skip the module declare function"));
        }
        if self.declare && self.evaluate {
            return Err(Error::new(
                span,
                "A function can be either declare or evaluate but not both.",
            ));
        }
        Ok(())
    }
}

pub(crate) enum ModuleFunctionOption {
    Declare(FlagOption<kw::declare>),
    Evaluate(FlagOption<kw::evaluate>),
    Skip(FlagOption<kw::skip>),
    Function(FunctionOption),
}

impl Parse for ModuleFunctionOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::declare) {
            input.parse().map(Self::Declare)
        } else if input.peek(kw::evaluate) {
            input.parse().map(Self::Evaluate)
        } else if input.peek(kw::skip) {
            input.parse().map(Self::Skip)
        } else {
            input.parse().map(Self::Function)
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct ModuleTypeConfig {
    pub skip: bool,
    pub class: ClassConfig,
    pub class_macro_name: Option<Path>,
}

impl ModuleTypeConfig {
    pub fn apply(&mut self, option: &ModuleTypeOption) {
        match option {
            ModuleTypeOption::Skip(x) => {
                self.skip = x.is_true();
            }
            ModuleTypeOption::Class(x) => self.class.apply(x),
        }
    }

    pub fn reexpand(&self) -> Option<Attribute> {
        let mut attrs = Vec::new();
        if self.class.frozen {
            attrs.push(quote!(frozen));
        }
        if let Some(x) = self.class.crate_.as_ref() {
            attrs.push(quote!(crate = #x));
        }
        if let Some(x) = self.class.rename.as_ref() {
            attrs.push(quote!(rename = #x));
        }
        if let Some(x) = self.class.rename_all {
            attrs.push(quote!(rename = #x));
        }

        if attrs.is_empty() {
            None
        } else if let Some(x) = self.class_macro_name.as_ref() {
            Some(parse_quote!(#![#x( #(#attrs,)* )]))
        } else {
            Some(parse_quote!(#![qjs( #(#attrs,)* )]))
        }
    }
}

pub(crate) enum ModuleTypeOption {
    Skip(FlagOption<kw::skip>),
    Class(ClassOption),
}

impl Parse for ModuleTypeOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::skip) {
            input.parse().map(Self::Skip)
        } else {
            input.parse().map(Self::Class)
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct ModuleItemConfig {
    pub skip: bool,
    pub rename: Option<String>,
}

impl ModuleItemConfig {
    pub fn apply(&mut self, option: &ModuleItemOption) {
        match option {
            ModuleItemOption::Skip(x) => {
                self.skip = x.is_true();
            }
            ModuleItemOption::Rename(x) => {
                self.rename = Some(x.value.value());
            }
        }
    }

    pub fn js_name(&self, name: &Ident, case: Option<Case>) -> String {
        if let Some(x) = self.rename.clone() {
            return x;
        }

        let name = name.to_string();
        if let Some(case) = case {
            return name.to_case(case.to_convert_case());
        }
        name
    }
}

pub(crate) enum ModuleItemOption {
    Skip(FlagOption<kw::skip>),
    Rename(ValueOption<kw::rename, LitStr>),
}

impl Parse for ModuleItemOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::skip) {
            input.parse().map(Self::Skip)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else {
            Err(syn::Error::new(
                input.span(),
                "invalid module item attribute",
            ))
        }
    }
}
