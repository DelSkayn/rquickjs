use super::{attrs::AttrVar, BindProp, Binder};
use crate::{config::Config, context::Source, Ident, TokenStream};
use quote::quote;
use syn::{Attribute, ImplItemConst, ItemConst, Visibility};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindConst {
    pub src: Source,
}

impl BindConst {
    pub fn set_src(&mut self, ident: &Ident, name: &str, new_src: Source) {
        if self.src == Default::default() {
            self.src = new_src;
        } else if self.src != new_src {
            error!(
                ident,
                "Attempt to redefine constant '{}' for `{}` which is already defined for `{}`",
                name,
                new_src,
                self.src
            );
        }
    }

    pub fn expand(&self, name: &str, cfg: &Config, is_module: bool) -> TokenStream {
        let exports_var = &cfg.exports_var;
        let pure = self.expand_pure(cfg);

        if is_module {
            quote! { #exports_var.export(#name, #pure)?; }
        } else {
            quote! { #exports_var.set(#name, #pure)?; }
        }
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
        let AttrVar {
            name,
            prop,
            writable,
            configurable,
            enumerable,
            proto,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

        if !self.visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let name = name.unwrap_or_else(|| ident.to_string());
        let src = self.sub_src(ident);

        if proto && !self.top_is_class() {
            error!(
                ident,
                "Unable to set module constant '{}' to prototype", name
            );
            return;
        }

        if prop {
            if let Some(prop) = self.top_item::<BindProp, _>(ident, &name, proto) {
                prop.set_const(ident, &name, BindConst { src });
                prop.set_writable(&name, writable);
                prop.set_configurable(configurable);
                prop.set_enumerable(enumerable);
            }
        } else if let Some(decl) = self.top_item::<BindConst, _>(ident, &name, proto) {
            decl.set_src(ident, &name, src);
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

            impl rquickjs::object::ObjectDef for Math {
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

            impl rquickjs::module::ModuleDef for Constants {
                fn declare(declares: &mut rquickjs::module::Declarations) -> rquickjs::Result<()>{
                    declares.declare("pi")?;
                    Ok(())
                }

                fn evaluate<'js>(_ctx: rquickjs::Ctx<'js>, exports: &mut rquickjs::module::Exports<'js>) -> rquickjs::Result<()>{
                    exports.export("pi", PI)?;
                    Ok(())
                }
            }
        };
        private_const {} {
            mod math {
                #[quickjs(value)]
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
