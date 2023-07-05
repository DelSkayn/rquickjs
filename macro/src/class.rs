use darling::{FromAttributes, FromField, FromMeta};
use proc_macro2::{Ident, Literal, TokenStream};
use proc_macro_error::abort;
use quote::quote;
use syn::{Fields, ItemStruct, Type, Visibility};

use crate::crate_ident;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

#[derive(Debug, FromMeta, FromField)]
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
    /// Don't trace this field
    skip_trace: bool,
    name: Option<Ident>,
    vis: Visibility,
    ty: Type,
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
}

pub(crate) fn expand(attr: AttrItem, item: ItemStruct) -> TokenStream {
    let ItemStruct {
        ref ident,
        ref generics,
        ref fields,
        ..
    } = item;

    let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
    let name = format!("{}", ident);
    let name = Literal::string(&name);

    let mutable = if attr.freeze {
        quote!(#lib_crate::class::Readable)
    } else {
        quote!(#lib_crate::class::Writable)
    };

    let _fields = Field::parse_fields(fields);

    // TODO properly figure out generics.
    quote! {
        #item

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

                let res = #lib_crate::Object::new(ctx)?;
                let implementor = #lib_crate::class::impl_::MethodImpl::<#ident>::new();
                #lib_crate::class::impl_::MethodImplementor::<#ident>::implement(&implementor,&res)?;
                Ok(Some(res))
            }
        }

        impl<'js> #generics #lib_crate::IntoJs<'js> for #ident #generics{
            fn into_js(self,ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                #lib_crate::IntoJs::into_js(#lib_crate::class::Class::<#ident>::instance(ctx,self)?, ctx)
            }
        }
    }
}
