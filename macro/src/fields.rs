use darling::FromField;
use proc_macro2::TokenStream;
use proc_macro_error::{abort, emit_warning};
use quote::{format_ident, quote};
use syn::{Attribute, Fields, Ident, Token, Type, Visibility};

#[derive(Debug, FromField)]
#[darling(attributes(qjs))]
pub(crate) struct Field {
    /// Rename the field when creating getters and setters.
    #[darling(default)]
    rename: Option<String>,
    /// Create a getter
    #[darling(default)]
    get: bool,
    /// Create a setter
    #[darling(default)]
    set: bool,

    #[darling(default)]
    enumerable: bool,
    #[darling(default)]
    configurable: bool,

    /// Don't trace this field
    #[darling(default)]
    skip_trace: bool,
    ident: Option<Ident>,
    vis: Visibility,
    ty: Type,
    attrs: Vec<Attribute>,
}

impl Field {
    pub fn parse_fields(fields: &Fields) -> Vec<Field> {
        match fields {
            Fields::Unit => Vec::new(),
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|x| match Field::from_field(x) {
                    Ok(x) => x,
                    Err(e) => {
                        abort!(x, "{}", e)
                    }
                })
                .collect(),
            Fields::Unnamed(fields) => fields
                .unnamed
                .iter()
                .map(|x| match Field::from_field(x) {
                    Ok(x) => x,
                    Err(e) => {
                        abort!(x, "{}", e)
                    }
                })
                .collect(),
        }
    }

    pub fn name(&self, which: usize) -> String {
        if let Some(name) = &self.rename {
            name.clone()
        } else {
            self.ident
                .clone()
                .map(|x| format!("{}", x))
                .unwrap_or_else(|| format!("{}", which))
        }
    }

    pub fn expand_prop_config(&self) -> TokenStream {
        let mut res = TokenStream::new();
        if self.configurable {
            res.extend(quote!(.configurable()));
        }
        if self.enumerable {
            res.extend(quote!(.enumerable()));
        }
        res
    }

    pub fn expand_trace_body(&self, lib_crate: &Ident, which: usize) -> TokenStream {
        if self.skip_trace {
            return TokenStream::new();
        }
        let field = self
            .ident
            .clone()
            .unwrap_or_else(|| format_ident!("{}", which));

        quote! {
            #lib_crate::class::Trace::<'js>::trace(&self.#field,_tracer);
        }
    }

    pub fn expand_property(&self, lib_crate: &Ident, which: usize) -> TokenStream {
        let accessor = if self.get && self.set {
            let field = self
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("{}", which));
            let ty = &self.ty;
            quote! {
                #lib_crate::object::Accessor::new(
                    |this: #lib_crate::function::This<#lib_crate::class::OwnedBorrow<'js, Self>>|{
                        this.0.#field.clone()
                    },
                    |mut this: #lib_crate::function::This<#lib_crate::class::OwnedBorrowMut<'js, Self>>, v: #ty|{
                        this.0.#field = v;
                    }
                )
            }
        } else if self.get {
            let field = self
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("{}", which));
            quote! {
                #lib_crate::object::Accessor::new_get(
                    |this: #lib_crate::function::This<#lib_crate::class::OwnedBorrow<'js, Self>>|{
                        this.0.#field.clone()
                    },
                )
            }
        } else if self.set {
            let field = self
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("{}", which));
            let ty = &self.ty;
            quote! {
                #lib_crate::object::Accessor::new_set(
                    |mut this: #lib_crate::function::This<#lib_crate::class::OwnedBorrowMut<'js, Self>>, v: #ty|{
                        this.0.#field = v;
                    }
                )
            }
        } else {
            return TokenStream::new();
        };
        let prop_config = self.expand_prop_config();
        let name = self.name(which);
        quote! {
            proto.prop(#name, #accessor #prop_config)?;
        }
    }

    pub fn expand_attrs(&self) -> TokenStream {
        if self.skip_trace {
            quote! {
                #[qjs(skip_trace)]
            }
        } else {
            TokenStream::new()
        }
    }

    pub fn expand_field(&self) -> TokenStream {
        let Field {
            ref ident,
            ref vis,
            ref ty,
            ref attrs,
            ..
        } = self;

        let rexported_attrs = self.expand_attrs();

        if let Some(ref ident) = ident {
            quote! {
                #(#attrs)*
                #rexported_attrs
                #vis #ident: #ty
            }
        } else {
            quote! {
                #(#attrs)*
                #rexported_attrs
                #vis #ty
            }
        }
    }
}
