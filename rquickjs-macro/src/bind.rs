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
mod class;
mod constant;
mod function;
mod module;
mod property;

use crate::{abort, error, Config, Ident, Source, TokenStream};
use darling::{util::Override, FromMeta};
use fnv::FnvBuildHasher;
use ident_case::RenameRule;
use indexmap::IndexMap;
use quote::{format_ident, quote};
use std::mem::replace;
use syn::{Attribute, ImplItem, Item, Visibility};

use attrs::*;
use class::*;
use constant::*;
use function::*;
use module::*;
use property::*;

pub use attrs::AttrItem;

pub type Map<K, V> = IndexMap<K, V, FnvBuildHasher>;

macro_rules! top_impls {
    ($($v:ident: $t:ident;)*) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        enum Top {
            $(
                $v($t),
            )*
        }

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
    Class: BindClass;
}

type BindConsts = Map<String, BindConst>;
type BindProps = Map<String, BindProp>;
type BindFns = Map<String, BindFn>;
type BindMods = Map<String, BindMod>;
type BindClasses = Map<String, BindClass>;

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
            Impl(item) => self.bind_impl(item),
            Struct(item) => self.bind_struct(item),
            Enum(item) => self.bind_enum(item),
            _ => (),
            //item => warning!("Unsupported item: {:?}"),
        }
    }

    pub fn bind_impl_item(&mut self, item: &mut ImplItem) {
        use ImplItem::*;
        match item {
            Const(item) => self.bind_impl_constant(item),
            Method(item) => self.bind_impl_function(item),
            _ => (),
            //item => warning!("Unsupported impl item: {:?}"),
        }
    }

    fn visible(&self, vis: &Visibility) -> bool {
        use Visibility::*;
        self.stack.is_empty() || matches!(vis, Public(_) | Crate(_))
    }

    fn top_src(&self) -> &Source {
        use Top::*;
        match &self.top {
            Mod(module) => &module.src,
            Class(class) => class.last_src(),
        }
    }

    fn top_is_impl(&self) -> bool {
        matches!(self.top, Top::Class(_))
    }

    fn top_consts(&mut self) -> &mut BindConsts {
        use Top::*;
        match &mut self.top {
            Mod(module) => &mut module.consts,
            Class(class) => &mut class.consts,
        }
    }

    fn top_props(&mut self) -> &mut BindProps {
        use Top::*;
        match &mut self.top {
            Mod(module) => &mut module.props,
            Class(datatype) => &mut datatype.props,
        }
    }

    fn top_prop(&mut self, name: &str) -> &mut BindProp {
        self.top_props()
            .entry(name.into())
            .or_insert_with(BindProp::default)
    }

    fn top_fns(&mut self) -> &mut BindFns {
        use Top::*;
        match &mut self.top {
            Mod(module) => &mut module.fns,
            Class(class) => &mut class.fns,
        }
    }

    fn top_mods(&mut self) -> &mut BindMods {
        use Top::*;
        match &mut self.top {
            Mod(module) => &mut module.mods,
            _ => unreachable!(),
        }
    }

    fn top_classes(&mut self) -> &mut BindClasses {
        use Top::*;
        match &mut self.top {
            Mod(module) => &mut module.classes,
            _ => unreachable!(),
        }
    }

    fn take_class(&mut self, name: &str) -> Option<BindClass> {
        self.top_classes().remove(name)
    }

    pub fn expand(
        &mut self,
        AttrItem {
            ident,
            init,
            module,
            object,
            test,
            public,
            ..
        }: AttrItem,
        mut item: Item,
    ) -> TokenStream {
        self.bind_item(&mut item);

        let lib_crate = &self.config.lib_crate;
        let exports = &self.config.exports_var;

        let bind_vis = match public {
            Some(Override::Inherit) => quote!(pub),
            Some(Override::Explicit(vis)) => quote!(pub(#vis)),
            _ => quote!(),
        };

        let def = if let Top::Mod(module) = &mut self.top {
            module
        } else {
            unreachable!();
        };

        if test {
            return def.object_init("test", &self.config);
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
            let name = ident.to_string();

            bindings.push(quote! {
                #bind_vis struct #ident;
            });

            if module {
                let mod_decl = def.module_decl(&name, &self.config);
                let mod_impl = def.module_impl(&name, &self.config);

                let mod_init = if let Some(init) = init {
                    let init_ident = init.unwrap_or_else(|| format_ident!("js_init_module"));
                    quote! {
                        #[no_mangle]
                        #bind_vis unsafe extern "C" fn #init_ident(
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
                let obj_init = def.object_init(&name, &self.config);

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
