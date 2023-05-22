#[cfg(test)]
macro_rules! test_cases {
    ($($c:ident { $($a:tt)* } { $($s:tt)* } { $($d:tt)* };)*) => {
        $(
            #[test]
            fn $c() {
                let mut binder = crate::Binder::new(crate::config::Config::default());
                let attrs: crate::AttributeArgs = syn::parse_quote! { $($a)* };
                let attrs = darling::FromMeta::from_list(&*attrs).unwrap();
                let input = syn::parse_quote! { $($s)* };
                let output = binder.expand(attrs, input);
                let actual = quote::quote! { #output };
                let expected = quote::quote! { $($d)* };
                assert_eq_tokens!(actual, expected);
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

use crate::{config::Config, context::Source, utils::PubVis, Ident, TokenStream};
use darling::FromMeta;
use fnv::FnvBuildHasher;
use ident_case::RenameRule;
use indexmap::IndexMap;
use quote::{format_ident, quote};
use std::{convert::TryFrom, mem::replace};
use syn::{spanned::Spanned, Attribute, ImplItem, Item, Visibility};

pub use attrs::AttrItem;

use self::{
    attrs::get_attrs, class::BindClass, constant::BindConst, function::BindFn, module::BindMod,
    property::BindProp,
};

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

macro_rules! item_impl {
	  ($($name:ident $type:ident,)*) => {
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum BindItem {
		        $(
                $name($type),
            )*
        }

        $(
            impl From<$type> for BindItem {
                fn from(value: $type) -> Self {
                    Self::$name(value)
                }
            }

            impl TryFrom<BindItem> for $type {
                type Error = BindItem;
                fn try_from(item: BindItem) -> Result<Self, Self::Error> {
                    if let BindItem::$name(value) = item {
                        Ok(value)
                    } else {
                        Err(item)
                    }
                }
            }

            impl<'a> TryFrom<&'a BindItem> for &'a $type {
                type Error = &'a BindItem;
                fn try_from(item: &'a BindItem) -> Result<Self, Self::Error> {
                    if let BindItem::$name(value) = item {
                        Ok(value)
                    } else {
                        Err(item)
                    }
                }
            }

            impl<'a> TryFrom<&'a mut BindItem> for &'a mut $type {
                type Error = &'a mut BindItem;
                fn try_from(item: &'a mut BindItem) -> Result<Self, Self::Error> {
                    if let BindItem::$name(value) = item {
                        Ok(value)
                    } else {
                        Err(item)
                    }
                }
            }
        )*

        impl BindItem {
            /*
            pub fn as_ref<'a, T>(&'a self) -> Option<&'a T>
            where
                &'a T: TryFrom<&'a Self>,
            {
                TryFrom::try_from(self).ok()
            }

            pub fn as_mut<'a, T>(&'a mut self) -> Option<&'a mut T>
            where
                &'a mut T: TryFrom<&'a mut Self>,
            {
                TryFrom::try_from(self).ok()
            }

            pub fn into_item<T>(self) -> Option<T>
            where
                T: TryFrom<Self>
            {
                TryFrom::try_from(self).ok()
            }
            */

            pub fn kind(&self) -> &'static str {
                match self {
                    $(
                        Self::$name(_) => stringify!($name),
                    )*
                }
            }
            pub fn expand(&self, name: &str, cfg: &Config, is_module: bool) -> TokenStream {
                match self {
                    $(
                        Self::$name(value) => value.expand(name, cfg,is_module),
                    )*
                }
            }
        }
	  };
}

item_impl! {
    Const BindConst,
    Prop BindProp,
    Fn BindFn,
    Mod BindMod,
    Class BindClass,
}

type BindItems = Map<String, BindItem>;

#[derive(Debug)]
pub struct Binder {
    config: Config,
    ident: Option<Ident>,
    src: Source,
    top: Top,
    stack: Vec<BindMod>,
}

impl Binder {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ident: None,
            top: Top::Mod(BindMod::default()),
            src: Source::default(),
            stack: Vec::new(),
        }
    }

    fn identify(&mut self, ident: &Ident) {
        if self.stack.is_empty() && self.ident.is_none() {
            self.ident = Some(ident.clone());
        }
    }

    pub fn get_attrs<R: FromMeta + Default + Extend<R>>(&self, attrs: &mut Vec<Attribute>) -> R {
        get_attrs(&self.config.bind_attr, attrs)
    }

    pub(super) fn hide_item(&self, attrs: &mut Vec<Attribute>, hide: bool) {
        if hide {
            attrs.push(syn::parse_quote! { #[cfg(all)] });
        }
    }

    pub fn bind_items(&mut self, items: &mut Vec<Item>) {
        for item in items {
            self.bind_item(item);
        }
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

    pub fn bind_impl_items(&mut self, items: &mut Vec<ImplItem>) {
        for item in items {
            self.bind_impl_item(item);
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
        self.src.is_empty() || matches!(vis, Public(_) | Crate(_))
    }

    fn top_src(&self) -> &Source {
        &self.src
    }

    fn sub_src(&self, ident: &Ident) -> Source {
        self.src.with_ident(ident.clone())
    }

    fn top_is_class(&self) -> bool {
        matches!(self.top, Top::Class(_))
    }

    fn top_class(&mut self) -> Option<&mut BindClass> {
        if let Top::Class(class) = &mut self.top {
            Some(class)
        } else {
            None
        }
    }

    fn top_items(&mut self, proto: bool) -> &mut BindItems {
        if proto {
            match &mut self.top {
                Top::Class(class) => &mut class.proto_items,
                _ => unreachable!(),
            }
        } else {
            match &mut self.top {
                Top::Mod(module) => &mut module.items,
                Top::Class(class) => &mut class.items,
            }
        }
    }

    fn top_item<T, S>(&mut self, span: S, name: &str, proto: bool) -> Option<&mut T>
    where
        T: Default,
        for<'a> &'a mut T: TryFrom<&'a mut BindItem, Error = &'a mut BindItem>,
        BindItem: From<T>,
        S: Spanned,
    {
        let item = self
            .top_items(proto)
            .entry(name.into())
            .or_insert_with(|| T::default().into());
        <&mut T>::try_from(item)
            .map_err(|item| {
                error!(
                    span.span(),
                    "The {} item is already defined with same name `{}`",
                    item.kind(),
                    name,
                );
            })
            .ok()
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
        let declares = &self.config.declare_var;

        let bind_vis = public.as_ref().map(PubVis::override_tokens);

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
                let mod_decl = def.module_decl(&self.config);
                let mod_impl = def.module_impl(&self.config);

                let mod_init = if let Some(init) = init {
                    let init_ident = init.unwrap_or_else(|| format_ident!("js_init_module"));
                    quote! {
                        #[no_mangle]
                        #bind_vis unsafe extern "C" fn #init_ident(
                            ctx: *mut #lib_crate::qjs::JSContext,
                            name: *const #lib_crate::qjs::c_char,
                        ) -> *mut #lib_crate::qjs::JSModuleDef {
                            #lib_crate::Module::init_raw::<#ident>(ctx, name)
                        }
                    }
                } else {
                    quote! {}
                };

                bindings.push(quote! {
                    impl #lib_crate::ModuleDef for #ident {
                        fn declare(#declares: &mut #lib_crate::Declarations) -> #lib_crate::Result<()> {
                            #mod_decl
                            Ok(())
                        }

                        fn evaluate<'js>(_ctx: #lib_crate::Ctx<'js>, #exports: &mut #lib_crate::Exports<'js>) -> #lib_crate::Result<()> {
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

    fn with_dir<F>(&mut self, ident: &Ident, func: F)
    where
        F: FnOnce(&mut Self),
    {
        let src = self.sub_src(ident);
        let src = replace(&mut self.src, src);
        func(self);
        self.src = src;
    }

    fn with_top<T, F>(&mut self, top: T, func: F) -> T
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

    fn with_item<T, F>(&mut self, ident: &Ident, name: &str, func: F)
    where
        T: From<Top> + Default + TryFrom<BindItem, Error = BindItem>,
        Top: From<T>,
        BindItem: From<T>,
        F: FnOnce(&mut Self),
    {
        let item = self.top_items(false).remove(name);
        let item = if let Some(item) = item {
            match T::try_from(item) {
                Ok(item) => item,
                Err(item) => {
                    error!(
                        ident,
                        "The {} item is already defined with same name `{}`",
                        item.kind(),
                        name,
                    );
                    self.top_items(false).insert(name.into(), item);
                    return;
                }
            }
        } else {
            T::default()
        };
        let item = self.with_top(item, func);
        self.top_items(false).insert(name.into(), item.into());
    }
}
