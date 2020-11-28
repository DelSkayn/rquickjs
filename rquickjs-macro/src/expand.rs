mod constant;
mod function;
mod module;

use crate::{abort, util::crate_name, Tokens};
use case::CaseExt;
use quote::{format_ident, quote};
use syn::{Ident, Item, Visibility};

pub struct Expander {
    /// rquickjs crate name
    pub lib_crate: Ident,

    /// exports ident (prototype object or module)
    pub exports: Ident,
}

impl Expander {
    pub fn new() -> Self {
        let lib_crate = match crate_name("rquickjs") {
            Ok(name) => format_ident!("{}", name),
            Err(error) => abort!("Unable to determine rquickjs crate name ({})", error),
        };
        let exports = format_ident!("exports");

        Self { lib_crate, exports }
    }

    fn module_def(&self, name: &Ident, decl: Option<Tokens>, init: Tokens) -> Tokens {
        let lib_crate = &self.lib_crate;
        let exports = &self.exports;

        let ident = format_ident!("{}", name.to_string().to_camel());
        let mut impls = Vec::new();

        impls.push(quote! {
            impl #lib_crate::ObjectDef for #ident {
                fn init<'js>(ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Object<'js>) -> #lib_crate::Result<()> {
                    #init
                    Ok(())
                }
            }
        });

        if let Some(decl) = decl {
            impls.push(quote! {
                impl #lib_crate::ModuleDef for #ident {
                    fn before_init<'js>(ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Module<'js, #lib_crate::BeforeInit>) -> #lib_crate::Result<()> {
                        #decl
                        Ok(())
                    }

                    fn after_init<'js>(ctx: #lib_crate::Ctx<'js>, #exports: &#lib_crate::Module<'js, #lib_crate::AfterInit>) -> #lib_crate::Result<()> {
                        #init
                        Ok(())
                    }
                }
            });
        }

        quote! {
            pub struct #ident;

            #(#impls)*
        }
    }

    /// Expand
    pub fn expand(&self, item: &Item) -> Tokens {
        let bindings = self.bindings(item);

        quote! {
            #item
            #bindings
        }
    }

    pub fn bindings(&self, item: &Item) -> Tokens {
        let path = Vec::new();
        let bindings = self
            .item(&path, item)
            .map(|(ident, decl, init)| self.module_def(ident, decl, init));
        quote! { #bindings }
    }

    pub fn is_visible(visibility: &Visibility) -> bool {
        use Visibility::*;
        match visibility {
            Public(_) | Crate(_) => true,
            _ => false,
        }
    }

    /// Expand item
    pub fn item<'a>(
        &self,
        path: &Vec<&Ident>,
        item: &'a Item,
    ) -> Option<(&'a Ident, Option<Tokens>, Tokens)> {
        let exports = &self.exports;
        Some(match item {
            Item::Const(item) if Self::is_visible(&item.vis) => (
                &item.ident,
                if path.is_empty() {
                    let name = item.ident.to_string();
                    Some(quote! { #exports.add(#name)?; })
                } else {
                    None
                },
                self.constant(path, item),
            ),
            Item::Fn(item) if Self::is_visible(&item.vis) => (
                &item.sig.ident,
                if path.is_empty() {
                    let name = item.sig.ident.to_string();
                    Some(quote! { #exports.add(#name)?; })
                } else {
                    None
                },
                self.function(path, item),
            ),
            Item::Mod(item) if Self::is_visible(&item.vis) => (
                &item.ident,
                if path.is_empty() {
                    Some(self.module_decl(item))
                } else {
                    None
                },
                self.module(path, item),
            ),
            _ => return None,
        })
    }

    /// Get item name
    pub fn item_ident<'a>(&self, item: &'a Item) -> Option<&'a Ident> {
        Some(match item {
            Item::Const(item) => &item.ident,
            Item::Fn(item) => &item.sig.ident,
            Item::Mod(item) => &item.ident,
            _ => return None,
        })
    }

    /// Expand path
    pub fn path(&self, path: &Vec<&Ident>, name: &Ident) -> Tokens {
        quote! {
            #(#path::)* #name
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::{format_ident, quote};
    use syn::parse_quote;

    #[test]
    fn initial_path() {
        let expander = Expander::new();
        let path = vec![];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn nested_path() {
        let expander = Expander::new();
        let seg1 = format_ident!("mod_a");
        let path = vec![&seg1];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            mod_a::name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn nested_nested_path() {
        let expander = Expander::new();
        let seg1 = format_ident!("mod_a");
        let seg2 = format_ident!("mod_b");
        let path = vec![&seg1, &seg2];
        let name = format_ident!("name");

        let actual = expander.path(&path, &name);
        let expected = quote! {
            mod_a::mod_b::name
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn simple_module() {
        let expander = Expander::new();
        let source = parse_quote! {
            #[allow(non_upper_case_globals)]
            pub mod native_module {
                pub const n: i32 = 123;
                pub const s: &str = "abc";
                pub fn f(a: f64, b: f64) -> f64 {
                    (a + b) * 0.5
                }
            }
        };

        let actual = expander.bindings(&source);
        let expected = quote! {
            pub struct NativeModule;

            impl rquickjs::ObjectDef for NativeModule {
                fn init<'js>(ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("n", native_module::n)?;
                    exports.set("s", native_module::s)?;
                    exports.set("f", rquickjs::JsFn::new("f", native_module::f))?;
                    Ok (())
                }
            }

            impl rquickjs::ModuleDef for NativeModule {
                fn before_init<'js>(ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::BeforeInit>) -> rquickjs::Result<()> {
                    exports.add("n")?;
                    exports.add("s")?;
                    exports.add("f")?;
                    Ok (())
                }

                fn after_init<'js>(ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Module<'js, rquickjs::AfterInit>) -> rquickjs::Result<()> {
                    exports.set("n", native_module::n)?;
                    exports.set("s", native_module::s)?;
                    exports.set("f", rquickjs::JsFn::new("f", native_module::f))?;
                    Ok (())
                }
            }
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
