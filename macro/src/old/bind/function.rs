use super::{attrs::AttrFn, BindProp, Binder};
use crate::{config::Config, context::Source, Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, FnArg, ImplItemMethod, ItemFn, Pat, Signature, Visibility};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindFn {
    pub fns: Vec<BindFn1>,
    pub class: Option<Source>,
}

impl BindFn {
    pub fn set_class(&mut self, ident: &Ident, name: &str, new_class: Source) {
        if let Some(class) = &self.class {
            if class != &new_class {
                error!(
                    ident,
                    "Attempt to overload constructor '{}' of class `{}` which is already defined for different class `{}`",
                    name,
                    new_class,
                    class
                );
            }
        } else {
            self.class = Some(new_class);
        }
    }

    pub fn func_name(&self, name: &str) -> String {
        /*if let Some(func) = self.fns.iter().next() {
            func.src
                .parent()
                .with_ident(format_ident!("{}", name))
                .to_string()
        } else {
            name.into()
        }*/
        name.into()
    }

    pub fn expand(&self, name: &str, cfg: &Config, is_module: bool) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let bindings = self
            .fns
            .iter()
            .map(|func| func.expand_pure(cfg))
            .collect::<Vec<_>>();
        let func_name = self.func_name(name);
        let bindings = match bindings.len() {
            0 => return quote! {},
            1 => quote! { #(#bindings)* },
            _ => quote! { (#(#bindings),*) },
        };
        let bindings = if let Some(class) = &self.class {
            quote! { #lib_crate::Class::<#class>::constructor(#bindings) }
        } else {
            bindings
        };
        if is_module {
            quote! { #exports_var.export(#name, #lib_crate::function::Func::new(#func_name, #bindings))?; }
        } else {
            quote! { #exports_var.set(#name, #lib_crate::function::Func::new(#func_name, #bindings))?; }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfArg {
    pub class: Source,
    pub self_: Ident,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindFn1 {
    pub src: Source,
    pub args: Vec<Ident>,
    pub define: Option<ItemFn>,
    pub async_: bool,
    pub method: bool,
    pub self_arg: Option<SelfArg>,
}

impl BindFn1 {
    pub fn new(src: &Source, ident: &Ident, args: &[Ident]) -> Self {
        Self {
            src: src.with_ident(ident.clone()),
            args: args.into(),
            ..Default::default()
        }
    }

    pub fn define(mut self, def: ItemFn) -> Self {
        self.define = Some(def);
        self
    }

    pub fn expand_pure(&self, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;

        let path = &self.src;
        let bind = if self.method {
            if let Some(self_arg) = self.self_arg.as_ref() {
                let cls = self_arg.class.clone();
                quote! { #lib_crate::function::SelfMethod::<#cls,_>::from(#path) }
            } else {
                quote! { #lib_crate::function::Method(#path) }
            }
        } else {
            quote! { #path }
        };
        let bind = if self.async_ {
            quote! { #lib_crate::function::Async(#bind) }
        } else {
            bind
        };
        if let Some(define) = &self.define {
            quote! {
                {
                    #define
                    #bind
                }
            }
        } else {
            bind
        }
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
            configurable,
            enumerable,
            ctor,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

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
        if ctor && !self.top_is_class() {
            error!(ident, "Constructor can be defined in impl block only");
        }

        let has_self = inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(_)));
        let method = self.top_is_class() && !ctor && has_self;

        self.identify(ident);

        let async_ = asyncness.is_some();
        let mut self_arg = None;

        let args = inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => {
                    self_arg = Some(SelfArg {
                        self_: format_ident!("self_"),
                        class: self.top_class().unwrap().src.clone(),
                    });
                    None
                }
                FnArg::Typed(arg) => match &*arg.pat {
                    Pat::Ident(pat) => Some(pat.ident.clone()),
                    _ => abort!(arg.colon_token, "Only named arguments is supported."),
                },
            })
            .collect::<Vec<_>>();

        let decl = BindFn1 {
            src: self.sub_src(ident),
            args,
            self_arg,
            async_,
            method,
            ..Default::default()
        };

        if get || set {
            if let Some(prop) = self.top_item::<BindProp, _>(ident, &name, method) {
                if get {
                    prop.set_getter(ident, &name, decl.clone());
                }
                if set {
                    prop.set_setter(ident, &name, decl);
                }
                prop.set_configurable(configurable);
                prop.set_enumerable(enumerable);
            }
        } else if ctor {
            let src = self.top_src().clone();
            if let Some(class) = self.top_class() {
                let func = class.ctor();
                func.set_class(ident, &name, src);
                func.fns.push(decl);
            }
        } else if let Some(func) = self.top_item::<BindFn, _>(ident, &name, method) {
            func.fns.push(decl);
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        no_args_no_return { test } {
            fn doit() {}
        } {
            exports.set("doit", rquickjs::function::Func::new("doit", doit))?;
        };

        overloaded_function { test } {
            #[quickjs(bare)]
            mod calc {
                #[quickjs(rename = "calc")]
                pub fn one() -> i32 { 1 }
                #[quickjs(rename = "calc")]
                pub fn inc(a: i32) -> i32 { a + 1 }
                #[quickjs(rename = "calc")]
                pub fn sum(a: i32, b: i32) -> i32 { a + b }
            }
        } {
            exports.set("calc", rquickjs::function::Func::new("calc", (calc::one, calc::inc, calc::sum)))?;
        };

        sync_function_object_export { object } {
            fn add2(a: f32, b: f32) -> f32 {
                a + b
            }
        } {
            fn add2(a: f32, b: f32) -> f32 {
                a + b
            }

            struct Add2;

            impl rquickjs::object::ObjectDef for Add2 {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("add2", rquickjs::function::Func::new("add2", add2))?;
                    Ok(())
                }
            }
        };

        async_function_object_export { object } {
            async fn fetch(url: String) -> Result<(i32, String)> {}
        } {
            async fn fetch(url: String) -> Result<(i32, String)> {}

            struct Fetch;

            impl rquickjs::object::ObjectDef for Fetch {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("fetch", rquickjs::function::Func::new("fetch", rquickjs::function::Async(fetch)))?;
                    Ok(())
                }
            }
        };
    }
}
