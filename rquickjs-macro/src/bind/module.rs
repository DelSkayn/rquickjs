use super::{AttrMod, BindClasses, BindConsts, BindFns, BindMods, BindProps, Binder};
use crate::{error, Config, Ident, Source, TokenStream};
use quote::quote;
use syn::ItemMod;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindMod {
    pub src: Source,
    pub bare: bool,
    pub consts: BindConsts,
    pub props: BindProps,
    pub fns: BindFns,
    pub mods: BindMods,
    pub classes: BindClasses,
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

    pub fn module_decl(&self, name: &str, cfg: &Config) -> TokenStream {
        let exports_var = &cfg.exports_var;

        if self.bare {
            let exports_list = self
                .consts
                .keys()
                .chain(self.props.keys())
                .chain(self.fns.keys())
                .chain(self.classes.keys())
                .map(|name| quote! { #exports_var.add(#name)?; })
                .chain(
                    self.mods
                        .iter()
                        .map(|(name, bind)| bind.module_decl(name, cfg)),
                );

            quote! { #(#exports_list)* }
        } else {
            quote! { #exports_var.add(#name)?; }
        }
    }

    pub fn module_impl(&self, name: &str, cfg: &Config) -> TokenStream {
        if self.bare {
            let exports_list = self
                .consts
                .iter()
                .map(|(name, bind)| bind.expand(name, cfg))
                .chain(self.fns.iter().map(|(name, bind)| bind.expand(name, cfg)))
                .chain(
                    self.classes
                        .iter()
                        .map(|(name, bind)| bind.expand(name, cfg)),
                )
                .chain(
                    self.mods
                        .iter()
                        .map(|(name, bind)| bind.module_impl(name, cfg)),
                );

            quote! { #(#exports_list)* }
        } else {
            self.object_init(name, cfg)
        }
    }

    pub fn object_init(&self, name: &str, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;

        let exports_list = self
            .consts
            .iter()
            .map(|(name, bind)| bind.expand(name, cfg))
            .chain(self.props.iter().map(|(name, bind)| bind.expand(name, cfg)))
            .chain(self.fns.iter().map(|(name, bind)| bind.expand(name, cfg)))
            .chain(
                self.classes
                    .iter()
                    .map(|(name, bind)| bind.expand(name, cfg)),
            )
            .chain(
                self.mods
                    .iter()
                    .map(|(name, bind)| bind.object_init(name, cfg)),
            );

        if self.bare {
            quote! { #(#exports_list)* }
        } else {
            quote! {
                #exports_var.set(#name, {
                    let #exports_var = #lib_crate::Object::new(_ctx)?;
                    #(#exports_list)*
                    #exports_var
                })?;
            }
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
        let AttrMod { name, bare, skip } = self.get_attrs(attrs);

        if content.is_none() || !self.visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let items = &mut content.as_mut().unwrap().1;
        let name = name.unwrap_or_else(|| ident.to_string());

        let src = self.top_src();
        let decl = BindMod::new(src, ident).bare(bare);

        let decl = self.with(decl, |this| {
            for item in items {
                this.bind_item(item);
            }
        });

        self.top_mods()
            .entry(name.clone())
            .and_modify(|def| {
                error!(
                    ident.span(),
                    "Module `{}` already defined with `{}`", name, def.src
                );
            })
            .or_insert(decl);
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        module_without_init { module } {
            #[quickjs(bare)]
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }
        } {
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }

            struct Lib;

            impl rquickjs::ModuleDef for Lib {
                fn load<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Created>) -> rquickjs::Result<()>{
                    exports.add("N")?;
                    exports.add("doit")?;
                    Ok(())
                }

                fn eval<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Loaded<rquickjs::Native>>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::JsFn::new("doit", lib::doit))?;
                    Ok(())
                }
            }
        };

        module_with_default_init { module, init } {
            #[quickjs(bare)]
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }
        } {
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }

            struct Lib;

            impl rquickjs::ModuleDef for Lib {
                fn load<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Created>) -> rquickjs::Result<()>{
                    exports.add("N")?;
                    exports.add("doit")?;
                    Ok(())
                }

                fn eval<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Loaded<rquickjs::Native>>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::JsFn::new("doit", lib::doit))?;
                    Ok(())
                }
            }

            #[no_mangle]
            unsafe extern "C" fn js_init_module(
                ctx: *mut rquickjs::qjs::JSContext,
                name: *const rquickjs::qjs::c_char,
            ) -> *mut rquickjs::qjs::JSModuleDef {
                rquickjs::Function::init_raw(ctx);
                rquickjs::Module::init::<Lib>(ctx, name)
            }
        };
    }
}
