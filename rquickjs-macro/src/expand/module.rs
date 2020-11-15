use crate::{abort, Expander, Tokens};
use quote::quote;
use syn::{Ident, ItemMod};

impl Expander {
    /// Expand module
    pub fn module(
        &self,
        path: &Vec<&Ident>,
        ItemMod { ident, content, .. }: &ItemMod,
        bare: bool,
    ) -> Tokens {
        if let Some((_, items)) = content {
            let lib_crate = &self.lib_crate;
            let name = ident.to_string();
            let path = {
                let mut path = path.clone();
                path.push(ident);
                path
            };
            let bindings = items
                .iter()
                .filter_map(|item| self.item(&path, item).map(|(_, tokens)| tokens))
                .collect::<Vec<_>>();

            if bare {
                quote! {
                    #(#bindings?;)*
                    Ok(())
                }
            } else {
                quote! {
                    obj.set(#name, {
                        let obj = #lib_crate::Object::new(ctx)?;
                        #(#bindings?;)*
                        obj
                    })
                }
            }
        } else {
            abort!(ident.span(), "Only modules with body can be binded.");
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::{format_ident, quote};
    use syn::parse_quote;

    #[test]
    fn mod_without_fns() {
        let item = parse_quote! {
            mod a {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.module(&path, &item, false);
        let expected = quote! {
            obj.set("a", {
                let obj = rquickjs::Object::new(ctx)?;
                obj
            })
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn bare_mod_without_fns() {
        let item = parse_quote! {
            mod a {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.module(&path, &item, true);
        let expected = quote! {
            Ok(())
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn mod_with_one_fn() {
        let item = parse_quote! {
            mod a {
                pub fn incr(val: i8) -> i8 { val + 1 }
            }
        };

        let expander = Expander::new();
        let path = Vec::new();
        let seg1 = format_ident!("a");
        let path1 = vec![&seg1];

        let actual = expander.module(&path, &item, false);
        let incr_fn = expander.function(
            &path1,
            &parse_quote! {
                fn incr(val: i8) -> i8 { val + 1 }
            },
        );
        let expected = quote! {
            obj.set("a", {
                let obj = rquickjs::Object::new(ctx)?;
                #incr_fn?;
                obj
            })
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn bare_mod_with_two_fns() {
        let item = parse_quote! {
            mod a {
                pub fn incr(val: i8) -> i8 { val + 1 }
                pub fn add2(a: f32, b: f32) -> f32 { a + b }
            }
        };

        let expander = Expander::new();
        let path = Vec::new();
        let seg1 = format_ident!("a");
        let path1 = vec![&seg1];

        let actual = expander.module(&path, &item, true);
        let incr_fn = expander.function(
            &path1,
            &parse_quote! {
                fn incr(val: i8) -> i8 { val + 1 }
            },
        );
        let add2_fn = expander.function(
            &path1,
            &parse_quote! {
                fn add2(a: f32, b: f32) -> f32 { a + b }
            },
        );
        let expected = quote! {
            #incr_fn?;
            #add2_fn?;
            Ok(())
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn mod_with_nested_with_one_fn() {
        let item = parse_quote! {
            mod a {
                pub mod b {
                    pub fn incr(val: i8) -> i8 { val + 1 }
                }
                pub fn add2(a: f32, b: f32) -> f32 { a + b }
            }
        };

        let expander = Expander::new();
        let path = Vec::new();
        let seg1 = format_ident!("a");
        let seg2 = format_ident!("b");
        let path1 = vec![&seg1];
        let path2 = vec![&seg1, &seg2];

        let actual = expander.module(&path, &item, false);
        let incr_fn = expander.function(
            &path2,
            &parse_quote! {
                fn incr(val: i8) -> i8 { val + 1 }
            },
        );
        let add2_fn = expander.function(
            &path1,
            &parse_quote! {
                fn add2(a: f32, b: f32) -> f32 { a + b }
            },
        );
        let expected = quote! {
            obj.set("a", {
                let obj = rquickjs::Object::new(ctx)?;
                obj.set("b", {
                    let obj = rquickjs::Object::new(ctx)?;
                    #incr_fn?;
                    obj
                })?;
                #add2_fn?;
                obj
            })
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
