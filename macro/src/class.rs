use darling::FromMeta;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Fields, Generics, ItemStruct, Lifetime, LifetimeParam};

use crate::{crate_ident, fields::Field};

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

pub fn add_js_lifetime(generics: &Generics) -> Generics {
    let mut generics = generics.clone();
    let has_js_lifetime = generics.lifetimes().any(|lt| lt.lifetime.ident == "'js");
    if has_js_lifetime {
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(LifetimeParam::new(Lifetime::new(
                "'js",
                Span::call_site(),
            ))),
        );
    }
    generics
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
            quote!(
            ( #(#fields),*)
            )
        }
        Fields::Unit => todo!(),
    };

    let trace_impls = prop_fields
        .iter()
        .enumerate()
        .map(|(idx, x)| x.expand_trace_body(&lib_crate, idx));

    let lifetime_generics = add_js_lifetime(generics);

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

            impl #lifetime_generics #lib_crate::class::JsClass<'js> for #ident #generics{
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
                    let implementor = #lib_crate::class::impl_::MethodImpl::<Self>::new();
                    (&implementor).implement(&proto)?;
                    Ok(Some(proto))
                }

                fn constructor(ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<Option<#lib_crate::function::Constructor<'js>>>{
                    use #lib_crate::class::impl_::ConstructorCreator;

                    let implementor = #lib_crate::class::impl_::ConstructorCreate::<Self>::new();
                    (&implementor).create_constructor(ctx)
                }
            }

            impl #lifetime_generics #lib_crate::IntoJs<'js> for #ident #generics{
                fn into_js(self,ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::IntoJs::into_js(#lib_crate::class::Class::<Self>::instance(ctx,self)?, ctx)
                }
            }

            impl #lifetime_generics #lib_crate::FromJs<'js> for #ident #generics
            where
                for<'a> CloneWrapper<'a,Self>: CloneTrait<Self>,
            {
                fn from_js(ctx: #lib_crate::Ctx<'js>, value: #lib_crate::Value<'js>) -> #lib_crate::Result<Self>{
                    let value = #lib_crate::class::Class::<Self>::from_js(ctx,value)?;
                    let borrow = value.try_borrow()?;
                    Ok(CloneWrapper(&*borrow).clone())
                }
            }
        }
    }
}
