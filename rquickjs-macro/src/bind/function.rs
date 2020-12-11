use super::{AttrFn, Binder};
use crate::{abort, error, Config, Ident, Source, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, FnArg, ImplItemMethod, ItemFn, Pat, Signature, Visibility};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindFn {
    pub src: Source,
    pub name: String,
    pub args: Vec<Ident>,
    pub asynch: bool,
    pub constr: bool,
    pub method: bool,
    pub define: Option<ItemFn>,
}

impl BindFn {
    pub fn new(src: &Source, ident: &Ident, name: &str, args: &[Ident]) -> Self {
        Self {
            src: src.with_ident(ident.clone()),
            name: name.into(),
            args: args.into(),
            ..Default::default()
        }
    }

    pub fn asynch(mut self, flag: bool) -> Self {
        self.asynch = flag;
        self
    }

    pub fn constr(mut self, flag: bool) -> Self {
        self.constr = flag;
        self
    }

    pub fn method(mut self, flag: bool) -> Self {
        self.method = flag;
        self
    }

    pub fn define(mut self, def: ItemFn) -> Self {
        self.define = Some(def);
        self
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let exports_var = &cfg.exports_var;
        let pure = self.expand_pure(cfg);

        quote! { #exports_var.set(#name, #pure)?; }
    }

    pub fn expand_pure(&self, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;

        let fn_name = &self.name;
        let path = &self.src;
        let bind = if self.asynch {
            let args = &self.args;
            quote! { |#(#args),*| #lib_crate::PromiseJs(#path(#(#args),*)) }
        } else {
            quote! { #path }
        };
        let bind = if self.constr {
            let class = path.parent();
            quote! { #lib_crate::Class::<#class>::constructor(#bind) }
        } else {
            bind
        };
        let bind = if self.method {
            quote! { #lib_crate::Method(#bind) }
        } else {
            bind
        };
        let bind = if let Some(define) = &self.define {
            quote! {
                {
                    #define
                    #bind
                }
            }
        } else {
            bind
        };
        quote! { #lib_crate::JsFn::new(#fn_name, #bind) }
    }
}

impl Binder {
    pub(super) fn bind_function(
        &mut self,
        ItemFn {
            attrs, vis, sig, ..
        }: &mut ItemFn,
    ) {
        self._bind_function(attrs, vis, sig);
    }

    pub(super) fn bind_impl_function(
        &mut self,
        ImplItemMethod {
            attrs, vis, sig, ..
        }: &mut ImplItemMethod,
    ) {
        self._bind_function(attrs, vis, sig);
    }

    fn _bind_function(
        &mut self,
        attrs: &mut Vec<Attribute>,
        vis: &Visibility,
        Signature {
            asyncness,
            unsafety,
            ident,
            inputs,
            variadic,
            ..
        }: &Signature,
    ) {
        let AttrFn {
            name,
            get,
            set,
            ctor,
            skip,
        } = self.get_attrs(attrs);

        if !self.visible(vis) || skip {
            return;
        }

        if let Some(unsafety) = unsafety {
            error!(
                unsafety,
                "Binding of unsafe functions is weird and not supported."
            );
            return;
        }
        if let Some(variadic) = variadic {
            error!(variadic, "Binding of variadic functions is not supported.");
            return;
        }

        let name = name.unwrap_or_else(|| ident.to_string());
        let ctor = ctor.unwrap_or_else(|| name == "new");
        if ctor && !self.top_is_impl() {
            error!(ident, "Constructor can be defined in impl block only");
        }

        let has_self = inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(_)));
        let method = self.top_is_impl() && !ctor && has_self;

        self.identify(ident);

        let asynch = asyncness.is_some();
        let args = inputs
            .iter()
            .map(|arg| match arg {
                FnArg::Receiver(_) => format_ident!("self_"),
                FnArg::Typed(arg) => match &*arg.pat {
                    Pat::Ident(pat) => pat.ident.clone(),
                    _ => abort!(arg.colon_token, "Only named arguments is supported."),
                },
            })
            .collect::<Vec<_>>();

        let src = self.top_src();
        let decl = BindFn::new(src, ident, &name, &args)
            .asynch(asynch)
            .constr(ctor)
            .method(method);

        if get.is_some() || set.is_some() {
            if let Some(name) = get {
                let prop = self.top_prop(&name);
                prop.set_getter(&ident, &name, decl.clone());
            }
            if let Some(name) = set {
                let prop = self.top_prop(&name);
                prop.set_setter(&ident, &name, decl);
            }
        } else {
            self.top_fns()
                .entry(name.clone())
                .and_modify(|def| {
                    error!(
                        ident,
                        "Function `{}` already defined with `{}`", name, def.src
                    );
                })
                .or_insert(decl);
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        no_args_no_return { test } {
            fn doit() {}
        } {
            exports.set("doit", rquickjs::JsFn::new("doit", doit))?;
        };

        sync_function { object } {
            fn add2(a: f32, b: f32) -> f32 {
                a + b
            }
        } {
            fn add2(a: f32, b: f32) -> f32 {
                a + b
            }

            struct Add2;

            impl rquickjs::ObjectDef for Add2 {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("add2" , rquickjs::JsFn::new("add2", add2))?;
                    Ok(())
                }
            }
        };
    }
}
