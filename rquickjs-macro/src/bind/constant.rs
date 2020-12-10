use super::{visible, AttrVar, BindMod, BindProp, Binder, Top};
use crate::{abort, Config, Ident, Source, TokenStream};
use quote::quote;
use syn::ItemConst;

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

    pub fn expand(&self, _cfg: &Config) -> TokenStream {
        let path = &self.src;
        quote! { #path }
    }
}

impl Binder {
    pub(super) fn bind_constant(
        &mut self,
        ItemConst {
            attrs, vis, ident, ..
        }: &mut ItemConst,
    ) {
        let AttrVar { name, prop, skip } = self.get_attrs(attrs);
        if !visible(vis) || skip {
            return;
        }

        self.identify(ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        if let Top::Mod(BindMod {
            src, consts, props, ..
        }) = &mut self.top
        {
            if prop {
                let BindProp { val, get, set, .. } =
                    props.entry(name.clone()).or_insert_with(BindProp::default);
                if let Some(val) = val {
                    abort!(
                        ident,
                        "Property `{}` already defined with const `{}`",
                        name,
                        val.src
                    );
                }
                if let Some(get) = get {
                    abort!(
                        ident,
                        "Property `{}` already defined with getter `{}`",
                        name,
                        get.src
                    );
                }
                if let Some(set) = set {
                    abort!(
                        ident,
                        "Property `{}` already defined with setter `{}`",
                        name,
                        set.src
                    );
                }
                *val = Some(BindConst::new(src, ident));
            } else {
                let cnst = BindConst::new(src, ident);
                consts.insert(name, cnst);
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        num_const { test } {
            pub const PI: f32 = core::math::f32::PI;
        } {
            exports.set("PI" , PI)?;
        };

        num_const_with_ident { object, ident = "Math" } {
            pub const PI: f32 = core::math::f32::PI;
        } {
            pub const PI: f32 = core::math::f32::PI;

            pub struct Math;

            impl rquickjs::ObjectDef for Math {
                fn init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()>{
                    exports.set("PI" , PI)?;
                    Ok(())
                }
            }
        };

        num_const_with_name { module, ident = "Constants" } {
            #[quickjs(name = "pi")]
            pub const PI: f32 = core::math::f32::PI;
        } {
            pub const PI: f32 = core::math::f32::PI;

            pub struct Constants;

            impl rquickjs::ModuleDef for Constants {
                fn before_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::BeforeInit>) -> rquickjs::Result<()>{
                    exports.add("pi")?;
                    Ok(())
                }

                fn after_init<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::AfterInit>) -> rquickjs::Result<()>{
                    exports.set("pi", PI)?;
                    Ok(())
                }
            }
        };
        private_const {} {
            #[quickjs(prop)]
            const PI: f32 = core::math::f32::PI;
        } {
            const PI: f32 = core::math::f32::PI;
        };
        skip_const {} {
            #[quickjs(skip)]
            pub const PI: f32 = core::math::f32::PI;
        } {
            pub const PI: f32 = core::math::f32::PI;
        };
    }
}
