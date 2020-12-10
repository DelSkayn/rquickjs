#[cfg(test)]
macro_rules! test_cases {
    ($($c:ident { $($a:tt)* } { $($s:tt)* } { $($d:tt)* };)*) => {
        $(
            #[test]
            fn $c() {
                let mut binder = crate::Binder::new(crate::Config::default());
                let attrs: crate::AttributeArgs = syn::parse_quote! { $($a)* };
                let attrs = darling::FromMeta::from_list(&*attrs).unwrap();
                let input = syn::parse_quote! { $($s)* };
                let output = binder.expand(attrs, input);
                let actual = quote::quote! { #output };
                let expected = quote::quote! { $($d)* };
                assert_eq!(actual.to_string(), expected.to_string());
            }
        )*
    };
}

mod attrs;
mod constant;
mod function;
mod module;
mod property;

use crate::{abort, error, Config, Ident, Source, TokenStream};
use darling::FromMeta;
use ident_case::RenameRule;
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::mem::replace;
use syn::{Attribute, Item, Visibility};

use attrs::*;
use constant::*;
use function::*;
use module::*;
use property::*;

pub use attrs::AttrItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BindType {
    src: Source,
    props: HashMap<String, BindProp>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BindImpl {
    src: Source,
    consts: HashMap<String, BindConst>,
    props: HashMap<String, BindProp>,
    fns: HashMap<String, BindFn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Top {
    Mod(BindMod),
    Type(BindType),
    Impl(BindImpl),
}

macro_rules! top_impls {
    ($($v:ident: $t:ident;)*) => {
        $(
            impl From<$t> for Top {
                fn from(x: $t) -> Self {
                    Top::$v(x)
                }
            }

            impl From<Top> for $t {
                fn from(x: Top) -> Self {
                    if let Top::$v(x) = x {
                        x
                    } else {
                        unreachable!();
                    }
                }
            }
        )*
    };
}

top_impls! {
    Mod: BindMod;
    Type: BindType;
    Impl: BindImpl;
}

#[derive(Debug)]
pub struct Binder {
    config: Config,
    ident: Option<Ident>,
    top: Top,
    stack: Vec<BindMod>,
}

impl Binder {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ident: None,
            top: Top::Mod(BindMod::root()),
            stack: Vec::new(),
        }
    }

    fn identify(&mut self, ident: &Ident) {
        if self.stack.is_empty() && self.ident.is_none() {
            self.ident = Some(ident.clone());
        }
    }

    pub fn get_attrs<R: FromMeta + Default + Merge>(&self, attrs: &mut Vec<Attribute>) -> R {
        get_attrs(&self.config.bind_attr, attrs)
    }

    pub fn bind_item(&mut self, item: &mut Item) {
        use Item::*;
        match item {
            Const(item) => self.bind_constant(item),
            Fn(item) => self.bind_function(item),
            Mod(item) => self.bind_module(item),
            _ => (),
        }
    }

    pub fn expand(
        &mut self,
        AttrItem {
            ident,
            init,
            module,
            object,
            test,
            ..
        }: AttrItem,
        mut item: Item,
    ) -> TokenStream {
        self.bind_item(&mut item);

        let lib_crate = &self.config.lib_crate;
        let exports = &self.config.exports_var;

        let def = if let Top::Mod(module) = &mut self.top {
            module
        } else {
            unreachable!();
        };

        if test {
            return def.object_init(&self.config);
        }

        let mut bindings = Vec::new();

        if module || object {
            let ident = if let Some(new_ident) = &ident {
                if let Some(ident) = &self.ident {
                    if new_ident == ident {
                        error!(ident.span(), "Binding ident conflict");
                    }
                }
                new_ident.clone()
            } else if let Some(ident) = &self.ident {
                let name = ident.to_string();
                let new_name = RenameRule::PascalCase.apply_to_field(&name);
                if new_name == name {
                    error!(
                        ident.span(),
                        "Binding ident conflict. Please add `ident = NewName` to binding attribute"
                    );
                }
                format_ident!("{}", new_name)
            } else {
                abort!("{}", "Unable to determine module name");
            };

            bindings.push(quote! {
                pub struct #ident;
            });

            if module {
                let mod_decl = def.module_decl(&self.config);
                let mod_impl = def.module_impl(&self.config);

                let mod_init = if let Some(init) = init {
                    let init_ident = init.unwrap_or_else(|| format_ident!("js_init_module"));
                    quote! {
                        #[no_mangle]
                        pub unsafe extern "C" fn #init_ident(
                            ctx: *mut #lib_crate::qjs::JSContext,
                            name: *const #lib_crate::qjs::c_char,
                        ) -> *mut #lib_crate::qjs::JSModuleDef {
                            #lib_crate::Function::init_raw(ctx);
                            #lib_crate::Module::init::<#ident>(ctx, name)
                        }
                    }
                } else {
                    quote! {}
                };

                bindings.push(quote! {
                    impl #lib_crate::ModuleDef for #ident {
                        fn before_init<'js>(_ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Module<'js, #lib_crate::BeforeInit>) -> #lib_crate::Result<()> {
                            #mod_decl
                            Ok(())
                        }

                        fn after_init<'js>(_ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Module<'js, #lib_crate::AfterInit>) -> #lib_crate::Result<()> {
                            #mod_impl
                            Ok(())
                        }
                    }

                    #mod_init
                });
            }

            if object {
                let obj_init = def.object_init(&self.config);

                bindings.push(quote! {
                    impl #lib_crate::ObjectDef for #ident {
                        fn init<'js>(_ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Object<'js>) -> #lib_crate::Result<()> {
                            #obj_init
                            Ok(())
                        }
                    }
                });
            }
        }

        quote! { #item #(#bindings)* }
    }

    fn with<T, F>(&mut self, top: T, func: F) -> T
    where
        T: From<Top>,
        Top: From<T>,
        F: FnOnce(&mut Self),
    {
        let top = replace(&mut self.top, top.into());
        self.stack.push(top.into());
        func(self);
        let top = self.stack.pop().unwrap().into();
        replace(&mut self.top, top).into()
    }
}

fn visible(vis: &Visibility) -> bool {
    use Visibility::*;
    matches!(vis, Public(_) | Crate(_))
}
