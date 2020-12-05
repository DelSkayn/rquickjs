use super::{visible, BindConst, BindFn, BindProp, Binder, Top};
use crate::{get_attrs, AttrMod, Config, Ident, Source, TokenStream};
use quote::quote;
use std::collections::HashMap;
use syn::ItemMod;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindMod {
    pub src: Source,
    pub bare: bool,
    pub consts: HashMap<String, BindConst>,
    pub props: HashMap<String, BindProp>,
    pub fns: HashMap<String, BindFn>,
    pub mods: HashMap<String, BindMod>,
}

impl BindMod {
    pub fn new(src: &Source, ident: &Ident) -> Self {
        Self {
            src: src.with_ident(ident.clone()),
            ..Default::default()
        }
    }

    pub fn root() -> Self {
        Self {
            bare: true,
            ..Default::default()
        }
    }

    pub fn bare(mut self, flag: bool) -> Self {
        self.bare = flag;
        self
    }

    pub fn module_decl(&self, cfg: &Config) -> TokenStream {
        let exports_var = &cfg.exports_var;
        let exports_list = self
            .consts
            .keys()
            .chain(self.props.keys())
            .chain(self.fns.keys())
            .chain(
                self.mods
                    .iter()
                    .filter(|(_, &BindMod { bare, .. })| !bare)
                    .map(|(name, _)| name),
            );

        let bare_exports = self
            .mods
            .iter()
            .filter(|(_, &BindMod { bare, .. })| bare)
            .map(|(_, bind)| bind.module_decl(cfg));

        quote! {
            #(#exports_var.add(#exports_list)?;)*
            #(#bare_exports)*
        }
    }

    pub fn module_impl(&self, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let exports_list = self
            .consts
            .iter()
            .map(|(name, bind)| {
                let value = bind.expand(cfg);
                quote! { #name, #value }
            })
            .chain(self.fns.iter().map(|(name, bind)| {
                let value = bind.expand(cfg);
                quote! { #name, #value }
            }))
            .chain(
                self.mods
                    .iter()
                    .filter(|(_, &BindMod { bare, .. })| !bare)
                    .map(|(name, bind)| {
                        let exports = bind.object_init(cfg);
                        quote! { #name, {
                            let #exports_var = #lib_crate::Object::new(_ctx)?;
                            #exports
                            #exports_var
                        } }
                    }),
            );

        let bare_exports = self
            .mods
            .iter()
            .filter(|(_, &BindMod { bare, .. })| bare)
            .map(|(_, bind)| bind.module_impl(cfg));

        quote! {
            #(#exports_var.set(#exports_list)?;)*
            #(#bare_exports)*
        }
    }

    pub fn object_init(&self, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let exports_list = self
            .consts
            .iter()
            .map(|(name, bind)| {
                let value = bind.expand(cfg);
                quote! { set(#name, #value) }
            })
            .chain(self.props.iter().map(|(name, bind)| {
                let value = bind.expand(cfg);
                quote! { prop(#name, #value) }
            }))
            .chain(self.fns.iter().map(|(name, bind)| {
                let value = bind.expand(cfg);
                quote! { set(#name, #value) }
            }))
            .chain(
                self.mods
                    .iter()
                    .filter(|(_, &BindMod { bare, .. })| !bare)
                    .map(|(name, bind)| {
                        let exports = bind.object_init(cfg);
                        quote! { set(#name, {
                            let #exports_var = #lib_crate::Object::new(_ctx)?;
                            #exports
                            #exports_var
                        }) }
                    }),
            );

        let bare_exports = self
            .mods
            .iter()
            .filter(|(_, &BindMod { bare, .. })| bare)
            .map(|(_, bind)| bind.object_init(cfg));

        quote! {
            #(#exports_var.#exports_list?;)*
            #(#bare_exports)*
        }
    }
}

impl Binder {
    pub(super) fn bind_module(
        &mut self,
        ItemMod {
            attrs,
            vis,
            ident,
            content,
            ..
        }: &mut ItemMod,
    ) {
        let AttrMod { name, bare, skip } = get_attrs(attrs);

        if content.is_none() || !visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let items = &mut content.as_mut().unwrap().1;
        let name = name.unwrap_or_else(|| ident.to_string());

        let module = if let Top::Mod(BindMod { src, .. }) = &self.top {
            BindMod::new(src, ident).bare(bare)
        } else {
            unreachable!();
        };

        let module = self.with(module, |this| {
            for item in items {
                this.bind_item(item);
            }
        });

        if let Top::Mod(BindMod { mods, .. }) = &mut self.top {
            mods.insert(name, module);
        } else {
            unreachable!();
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        module_without_init { module } {
            #[bind(bare)]
            pub mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }
        } {
            pub mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }

            pub struct Lib;

            impl rquickjs::ModuleDef for Lib {
                fn before_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::BeforeInit>) -> rquickjs::Result<()>{
                    exports.add("N")?;
                    exports.add("doit")?;
                    Ok(())
                }

                fn after_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::AfterInit>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::JsFn::new("doit", lib::doit))?;
                    Ok(())
                }
            }
        };

        module_with_default_init { module, init } {
            #[bind(bare)]
            pub mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }
        } {
            pub mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }

            pub struct Lib;

            impl rquickjs::ModuleDef for Lib {
                fn before_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::BeforeInit>) -> rquickjs::Result<()>{
                    exports.add("N")?;
                    exports.add("doit")?;
                    Ok(())
                }

                fn after_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::AfterInit>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::JsFn::new("doit", lib::doit))?;
                    Ok(())
                }
            }

            #[no_mangle]
            pub unsafe extern "C" fn js_init_module(
                ctx: *mut rquickjs::qjs::JSContext,
                name: *const rquickjs::qjs::c_char,
            ) -> *mut rquickjs::qjs::JSModuleDef {
                rquickjs::Function::init_raw(ctx);
                rquickjs::Module::init::<Lib>(ctx, name)
            }
        };
    }
}
