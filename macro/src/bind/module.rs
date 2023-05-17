use super::{AttrMod, BindItems, Binder};
use crate::{Config, TokenStream};
use quote::quote;
use syn::ItemMod;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindMod {
    pub items: BindItems,
}

impl BindMod {
    pub fn module_decl(&self, cfg: &Config) -> TokenStream {
        let define_var = &cfg.define_var;
        let exports_list = self
            .items
            .keys()
            .map(|name| quote! { #define_var.define(#name)?; });

        quote! { #(#exports_list)* }
    }

    pub fn module_impl(&self, cfg: &Config) -> TokenStream {
        let exports_list = self.items.iter().map(|(name, bind)| bind.expand(name, cfg));

        quote! { #(#exports_list)* }
    }

    pub fn object_init(&self, _name: &str, cfg: &Config) -> TokenStream {
        let exports_list = self.items.iter().map(|(name, bind)| bind.expand(name, cfg));
        quote! { #(#exports_list)* }
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let bindings = self.object_init(name, cfg);
        quote! {
            #exports_var.export(#name, {
               let #exports_var = #lib_crate::Object::new(_ctx)?;
                #bindings
                #exports_var
            })?;
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
        let AttrMod {
            name,
            bare,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

        if content.is_none() || !self.visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let items = &mut content.as_mut().unwrap().1;
        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_dir(ident, |this| {
            if bare {
                this.bind_items(items);
            } else {
                this.with_item::<BindMod, _>(ident, &name, |this| {
                    this.bind_items(items);
                });
            }
        });
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        no_bare_module_without_init { module } {
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
                fn define<'js>(exports: &mut rquickjs::Definition) -> rquickjs::Result<()>{
                    exports.add("lib")?;
                    Ok(())
                }

                fn eval<'js>(_ctx: rquickjs::Ctx<'js>, exports: &mut Exports<'js>) -> rquickjs::Result<()>{
                    exports.export("lib", {
                        let exports = rquickjs::Object::new(_ctx)?;
                        exports.set("N", lib::N)?;
                        exports.set("doit", rquickjs::Func::new("doit", lib::doit))?;
                        exports
                    })?;
                    Ok(())
                }
            }
        };

        no_bare_object_public_crate { object, public = "crate" } {
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }
        } {
            mod lib {
                pub const N: i8 = 3;
                pub fn doit() {}
            }

            pub(crate) struct Lib;

            impl rquickjs::ObjectDef for Lib {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("lib", {
                        let exports = rquickjs::Object::new(_ctx)?;
                        exports.set("N", lib::N)?;
                        exports.set("doit", rquickjs::Func::new("doit", lib::doit))?;
                        exports
                    })?;
                    Ok(())
                }
            }
        };

        bare_object_public { object, public } {
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

            pub struct Lib;

            impl rquickjs::ObjectDef for Lib {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::Func::new("doit", lib::doit))?;
                    Ok(())
                }
            }
        };

        bare_module_without_init { module } {
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
                fn load<'js>(exports: &rquickjs::Module<'js, rquickjs::Created>) -> rquickjs::Result<()>{
                    exports.add("N")?;
                    exports.add("doit")?;
                    Ok(())
                }

                fn eval<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Loaded<rquickjs::Native>>) -> rquickjs::Result<()>{
                    exports.set("N", lib::N)?;
                    exports.set("doit", rquickjs::Func::new("doit", lib::doit))?;
                    Ok(())
                }
            }
        };

        bare_module_with_default_init { module, init } {
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
                    exports.set("doit", rquickjs::Func::new("doit", lib::doit))?;
                    Ok(())
                }
            }

            #[no_mangle]
            unsafe extern "C" fn js_init_module(
                ctx: *mut rquickjs::qjs::JSContext,
                name: *const rquickjs::qjs::c_char,
            ) -> *mut rquickjs::qjs::JSModuleDef {
                rquickjs::Module::init_raw::<Lib>(ctx, name)
            }
        };
    }
}
