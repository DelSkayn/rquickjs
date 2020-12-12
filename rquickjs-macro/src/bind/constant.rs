use super::{AttrVar, Binder};
use crate::{error, Config, Ident, Source, TokenStream};
use quote::quote;
use syn::{Attribute, ImplItemConst, ItemConst, Visibility};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindConst {
    pub src: Source,
}

impl BindConst {
    pub fn new(src: &Source, ident: &Ident) -> Self {
        Self {
            src: src.with_ident(ident.clone()),
        }
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let exports_var = &cfg.exports_var;
        let pure = self.expand_pure(cfg);

        quote! { #exports_var.set(#name, #pure)?; }
    }

    pub fn expand_pure(&self, _cfg: &Config) -> TokenStream {
        let src = &self.src;

        quote! { #src }
    }
}

impl Binder {
    pub(super) fn bind_constant(
        &mut self,
        ItemConst {
            attrs, vis, ident, ..
        }: &mut ItemConst,
    ) {
        self._bind_constant(attrs, vis, ident);
    }

    pub(super) fn bind_impl_constant(
        &mut self,
        ImplItemConst {
            attrs, vis, ident, ..
        }: &mut ImplItemConst,
    ) {
        self._bind_constant(attrs, vis, ident);
    }

    fn _bind_constant(&mut self, attrs: &mut Vec<Attribute>, vis: &Visibility, ident: &Ident) {
        let AttrVar { name, prop, skip } = self.get_attrs(attrs);
        if !self.visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let name = name.unwrap_or_else(|| ident.to_string());
        let src = self.top_src();
        let decl = BindConst::new(src, ident);

        if prop {
            self.top_prop(&name).set_const(&ident, &name, decl);
        } else {
            self.top_consts()
                .entry(name.clone())
                .and_modify(|def| {
                    error!(
                        ident,
                        "Constant `{}` already defined with `{}`", name, def.src
                    );
                })
                .or_insert(decl);
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        num_const { test } {
            const PI: f32 = core::math::f32::PI;
        } {
            exports.set("PI" , PI)?;
        };

        num_const_with_ident { object, ident = "Math" } {
            const PI: f32 = core::math::f32::PI;
        } {
            const PI: f32 = core::math::f32::PI;

            struct Math;

            impl rquickjs::ObjectDef for Math {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("PI" , PI)?;
                    Ok(())
                }
            }
        };

        num_const_with_name { module, ident = "Constants" } {
            #[quickjs(rename = "pi")]
            const PI: f32 = core::math::f32::PI;
        } {
            const PI: f32 = core::math::f32::PI;

            struct Constants;

            impl rquickjs::ModuleDef for Constants {
                fn load<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Created>) -> rquickjs::Result<()>{
                    exports.add("pi")?;
                    Ok(())
                }

                fn eval<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::Loaded<rquickjs::Native>>) -> rquickjs::Result<()>{
                    exports.set("pi", PI)?;
                    Ok(())
                }
            }
        };
        private_const {} {
            mod math {
                #[quickjs(property)]
                const PI: f32 = core::math::f32::PI;
            }
        } {
            mod math {
                const PI: f32 = core::math::f32::PI;
            }
        };
        skip_const {} {
            #[quickjs(skip)]
            const PI: f32 = core::math::f32::PI;
        } {
            const PI: f32 = core::math::f32::PI;
        };
    }
}
