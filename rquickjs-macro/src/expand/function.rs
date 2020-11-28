use crate::{abort, Expander, Tokens};
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, FnArg, Ident, ItemFn, Pat, Signature, Token};

impl Expander {
    /// Expand function
    pub fn function(
        &self,
        path: &Vec<&Ident>,
        ItemFn {
            sig:
                Signature {
                    asyncness,
                    unsafety,
                    ident,
                    inputs,
                    variadic,
                    //output,
                    ..
                },
            ..
        }: &ItemFn,
    ) -> Tokens {
        if let Some(unsafety) = unsafety {
            abort!(
                unsafety.span,
                "Binding of unsafe functions is weird and not supported."
            );
        }
        if let Some(variadic) = variadic {
            abort!(
                variadic.dots.spans[0],
                "Binding of variadic functions is not supported."
            );
        }
        let is_async = asyncness.is_some();

        let lib_crate = &self.lib_crate;
        let exports = &self.exports;

        let name = format!("{}", ident);
        let path = self.path(path, ident);

        let arg_names = self.arg_names(inputs);

        let fn_decl = if is_async {
            quote! {
                |#(#arg_names),*|
                #lib_crate::PromiseJs(#path(#(#arg_names),*))
            }
        } else {
            quote! { #path }
        };

        quote! {
            #exports.set(#name, #lib_crate::JsFn::new(#name, #fn_decl))?;
        }
    }

    /// Extract function arguments names (excluding self)
    pub fn arg_names(&self, args: &Punctuated<FnArg, Token![,]>) -> Vec<Ident> {
        args.iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => Some(format_ident!("self_")),
                FnArg::Typed(arg) => Some(match &*arg.pat {
                    Pat::Ident(pat) => pat.ident.clone(),
                    _ => abort!(arg.colon_token, "Only named arguments is supported."),
                }),
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn fn_no_args_no_return() {
        let item = parse_quote! {
            fn doit() {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("doit", rquickjs::JsFn::new("doit", doit))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_no_args() {
        let item = parse_quote! {
            fn geti() -> i32 { 1 }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("geti", rquickjs::JsFn::new("geti", geti))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_one_arg() {
        let item = parse_quote! {
            fn incr(val: i32) -> i32 { val + 1 }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("incr", rquickjs::JsFn::new("incr", incr))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_two_args() {
        let item = parse_quote! {
            fn add2(a: i32, b: i32) -> i32 { a + b }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("add2", rquickjs::JsFn::new("add2", add2))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn async_fn_no_args_no_return() {
        let item = parse_quote! {
            async fn doit() {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("doit", rquickjs::JsFn::new("doit", | | rquickjs::PromiseJs(doit())))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn async_fn_two_args_no_throw() {
        let item = parse_quote! {
            async fn mul2(a: f64, b: f64) -> f64 {
                a * b
            }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            exports.set("mul2", rquickjs::JsFn::new("mul2", |a, b| rquickjs::PromiseJs(mul2(a, b))))?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
