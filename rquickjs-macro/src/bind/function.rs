use super::{visible, BindMod, BindProp, Binder, Top};
use crate::{abort, error, get_attrs, AttrFn, Config, Ident, Source, TokenStream};
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, Signature};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindFn {
    pub src: Source,
    pub name: String,
    pub args: Vec<Ident>,
    pub asynch: bool,
    pub constr: bool,
    pub method: bool,
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

    pub fn expand(&self, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let name = &self.name;
        let path = &self.src;
        let bind = if self.asynch {
            let args = &self.args;
            quote! { |#(#args),*| #lib_crate::PromiseJs(#path(#(#args),*)) }
        } else {
            quote! { #path }
        };
        //if self.constr || self.method
        quote! { #lib_crate::JsFn::new(#name, #bind) }
    }
}

impl Binder {
    pub(super) fn bind_function(
        &mut self,
        ItemFn {
            attrs,
            vis,
            sig:
                Signature {
                    asyncness,
                    unsafety,
                    ident,
                    inputs,
                    variadic,
                    ..
                },
            ..
        }: &mut ItemFn,
    ) {
        let AttrFn {
            name,
            get,
            set,
            ctor,
            skip,
        } = get_attrs(attrs);

        if !visible(vis) || skip {
            return;
        }

        if let Some(unsafety) = unsafety {
            error!(
                unsafety.span,
                "Binding of unsafe functions is weird and not supported."
            );
            return;
        }
        if let Some(variadic) = variadic {
            error!(
                variadic.dots.spans[0],
                "Binding of variadic functions is not supported."
            );
            return;
        }

        self.identify(ident);

        let asynch = asyncness.is_some();
        let name = name.unwrap_or_else(|| ident.to_string());
        let args = inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => Some(format_ident!("self_")),
                FnArg::Typed(arg) => Some(match &*arg.pat {
                    Pat::Ident(pat) => pat.ident.clone(),
                    _ => abort!(arg.colon_token, "Only named arguments is supported."),
                }),
            })
            .collect::<Vec<_>>();

        if let Top::Mod(BindMod {
            src, fns, props, ..
        }) = &mut self.top
        {
            if ctor {
                abort!(ident, "Constructor cannot be defined for module");
            }

            macro_rules! xetter {
                ($($f:ident)*) => {
                    $(
                        if let Some(prop) = &$f {
                            let BindProp { val, $f, .. } = props
                                .entry(prop.clone())
                                .or_insert_with(BindProp::default);
                            if let Some(val) = val {
                                abort!(ident, "Property `{}` already defined with const `{}`", name, val.src);
                            }
                            if let Some(fun) = $f {
                                abort!(ident, "Property `{}` already has getter `{}`", name, fun.src);
                            }
                            *$f = Some(BindFn::new(src, ident, &name, &args).asynch(asynch));
                        }
                    )*
                };
            }

            if get.is_some() || set.is_some() {
                xetter!(get set);
            } else {
                if fns.contains_key(&name) {
                    abort!(ident, "Function `{}` already defined", name);
                }
                let func = BindFn::new(src, ident, &name, &args).asynch(asynch);
                fns.insert(name, func);
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        no_args_no_return { test } {
            pub fn doit() {}
        } {
            exports.set("doit", rquickjs::JsFn::new("doit", doit))?;
        };

        sync_function { object } {
            pub fn add2(a: f32, b: f32) -> f32 {
                a + b
            }
        } {
            pub fn add2(a: f32, b: f32) -> f32 {
                a + b
            }

            pub struct Add2;

            impl rquickjs::ObjectDef for Add2 {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("add2" , rquickjs::JsFn::new("add2", add2))?;
                    Ok(())
                }
            }
        };
    }
}
