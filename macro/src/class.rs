use darling::{FromField, FromMeta};
use proc_macro2::{Ident, Literal, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{Attribute, Fields, ItemStruct, Type, Visibility};

use crate::crate_ident;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

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

    pub fn expand_field(&self) -> TokenStream {
        let Field {
            ref ident,
            ref vis,
            ref ty,
            ref attrs,
            ..
        } = self;
        if let Some(ref ident) = ident {
            quote! {
                #(#attrs)*
                #vis #ident: #ty
            }
        } else {
            quote! {
                #(#attrs)*
                #vis #ty
            }
        }
    }
}

pub(crate) fn expand(attr: AttrItem, item: ItemStruct) -> TokenStream {
    let ItemStruct {
        ref ident,
        ref generics,
        ref fields,
        ref attrs,
        ref vis,
        ref struct_token,
        ref semi_token,
    } = item;

    let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
    let name = format!("{}", ident);
    let name = Literal::string(&name);

    let mutable = if attr.freeze {
        quote!(#lib_crate::class::Readable)
    } else {
        quote!(#lib_crate::class::Writable)
    };

    let prop_fields = Field::parse_fields(fields);
    let props = prop_fields
        .iter()
        .enumerate()
        .map(|(idx, x)| x.expand_property(&lib_crate, idx));

    let impl_mod = format_ident!("__impl__{}", ident);

    let fields = match fields {
        Fields::Named(_) => {
            let fields = prop_fields.iter().map(|x| x.expand_field());
            quote!({
                #(#fields),*
            })
        }
        Fields::Unnamed(_) => {
            let fields = prop_fields.iter().map(|x| x.expand_field());
            quote!((
                #(#fields),*
            ))
        }
        Fields::Unit => todo!(),
    };

    // TODO properly figure out generics.
    quote! {
        #(#attrs)*
        #vis #struct_token #ident #generics #fields #semi_token

        #[allow(non_snake_case)]
        mod #impl_mod{
            pub use super::*;

            struct CloneWrapper<'a,T>(&'a T);
            /// A helper trait to implement FromJs for types which implement clone.
            trait CloneTrait<T>{
                fn clone(&self) -> T;
            }

            impl<'a, T: Clone> CloneTrait<T> for CloneWrapper<'a,T>{
                fn clone(&self) -> T{
                    self.0.clone()
                }
            }

            impl<'js> #generics #lib_crate::class::Trace<'js> for #ident #generics{
                fn trace<'a>(&self, _: #lib_crate::class::Tracer<'a,'js>){
                }
            }

            impl<'js> #generics #lib_crate::class::JsClass<'js> for #ident #generics{
                const NAME: &'static str = #name;

                type Mutable = #mutable;

                fn class_id() -> &'static #lib_crate::class::ClassId{
                    static ID: #lib_crate::class::ClassId =  #lib_crate::class::ClassId::new();
                    &ID
                }

                fn prototype(ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<Option<#lib_crate::Object<'js>>>{
                    use #lib_crate::class::impl_::MethodImplementor;

                    let proto = #lib_crate::Object::new(ctx)?;
                    #(#props)*
                    let implementor = #lib_crate::class::impl_::MethodImpl::<#ident>::new();
                    (&&implementor).implement(&proto)?;
                    Ok(Some(proto))
                }
            }

            impl<'js> #generics #lib_crate::IntoJs<'js> for #ident #generics{
                fn into_js(self,ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::IntoJs::into_js(#lib_crate::class::Class::<#ident>::instance(ctx,self)?, ctx)
                }
            }

            impl<'js> #generics #lib_crate::FromJs<'js> for #ident #generics
            where
                for<'a> CloneWrapper<'a,Self>: CloneTrait<Self>,
            {
                fn from_js(ctx: #lib_crate::Ctx<'js>, value: #lib_crate::Value<'js>) -> #lib_crate::Result<#ident>{
                    let value = #lib_crate::class::Class::<Self>::from_js(ctx,value)?;
                    let borrow = value.try_borrow()?;
                    Ok(CloneWrapper(&*borrow).clone())
                }
            }
        }
    }
}
