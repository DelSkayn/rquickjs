use super::{data_fields, has_lifetime, new_lifetime, DataContent};
use crate::{abort, Config, Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Index};

pub struct IntoJs {
    config: Config,
}

impl IntoJs {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn expand(
        &self,
        DeriveInput {
            //attrs,
            ident,
            generics,
            data,
            ..
        }: DeriveInput,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let typename = quote! { #ident #generics };
        let params = &generics.params;
        let where_clause = &generics.where_clause;

        let js_lt = new_lifetime("'js");
        let impl_params = quote! { #params };
        let impl_params = if has_lifetime(&params, &js_lt) {
            impl_params
        } else {
            quote! { #js_lt, #impl_params }
        };

        use Data::*;
        let body = match data {
            Struct(data) => self.from_fields(&ident, None, &data.fields),
            Enum(data) => {
                let matches = data
                    .variants
                    .iter()
                    .map(|variant| self.from_fields(&ident, Some(&variant.ident), &variant.fields));

                quote! {
                    match self {
                        #(#matches)*
                    }
                }
            }
            _ => abort!(ident.span(), "Only structs and enums are supported"),
        };

        quote! {
            impl<#impl_params> #lib_crate::IntoJs<#js_lt> for #typename #where_clause {
                fn into_js(self, ctx: #lib_crate::Ctx<#js_lt>) -> #lib_crate::Result<#lib_crate::Value<#js_lt>> {
                    #body
                }
            }
        }
    }

    fn from_fields(&self, ident: &Ident, variant: Option<&Ident>, fields: &Fields) -> TokenStream {
        let lib_crate = &self.config.lib_crate;

        use DataContent::*;
        let ctor = variant
            .map(|variant| quote! { #ident::#variant })
            .unwrap_or_else(|| quote! { #ident });
        match data_fields(fields) {
            Fields(fields) => {
                let field_names = fields.iter().map(|field| field.to_string());
                if variant.is_some() {
                    let field_aliases = fields.iter().map(|field| format_ident!("_{}", field));
                    let field_aliases2 = fields.iter().map(|field| format_ident!("_{}", field));
                    quote! {
                        #ctor { #(#fields: #field_aliases),* } => {
                            let val = #lib_crate::Object::new(ctx)?;
                            #(val.set(#field_names, #field_aliases2)?;)*
                            Ok(val.into_value())
                        }
                    }
                } else {
                    quote! {
                        let val = #lib_crate::Object::new(ctx)?;
                        #(val.set(#field_names, self.#fields)?;)*
                        Ok(val.into_value())
                    }
                }
            }
            Points(count) => {
                let point_indexes = (0..count).map(Index::from);
                if variant.is_some() {
                    let point_aliases = (0..count).map(|index| format_ident!("_{}", index));
                    let point_aliases2 = (0..count).map(|index| format_ident!("_{}", index));
                    quote! {
                        #ctor (#(#point_aliases),*) => {
                            let val = #lib_crate::Array::new(ctx)?;
                            #(val.set(#point_indexes, #point_aliases2)?;)*
                            Ok(val.into_value())
                        }
                    }
                } else {
                    let point_indexes2 = (0..count).map(Index::from);
                    quote! {
                        let val = #lib_crate::Array::new(ctx)?;
                        #(val.set(#point_indexes, self.#point_indexes2)?;)*
                        Ok(val.into_value())
                    }
                }
            }
            Nothing => {
                if let Some(variant) = variant {
                    let name = variant.to_string();
                    quote! {
                        #ctor => #name.into_js(ctx),
                    }
                } else {
                    quote! {
                        Ok(#lib_crate::Value::Undefined)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        unit_struct IntoJs {
            struct SomeStruct;
        } {
            impl<'js,> rquickjs::IntoJs<'js> for SomeStruct {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(rquickjs::Value::Undefined)
                }
            }
        };

        tuple_struct IntoJs {
            struct Struct(i32, String);
        } {
            impl<'js,> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let val = rquickjs::Array::new(ctx)?;
                    val.set(0, self.0)?;
                    val.set(1, self.1)?;
                    Ok(val.into_value())
                }
            }
        };

        struct_with_fields IntoJs {
            struct Struct {
                int: i32,
                text: String,
            }
        } {
            impl<'js,> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let val = rquickjs::Object::new(ctx)?;
                    val.set("int", self.int)?;
                    val.set("text", self.text)?;
                    Ok(val.into_value())
                }
            }
        };

        unit_enum IntoJs {
            enum Enum {
                A,
                B,
            }
        } {
            impl<'js,> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    match self {
                        Enum::A => "A".into_js(ctx),
                        Enum::B => "B".into_js(ctx),
                    }
                }
            }
        };

        tuple_enum IntoJs {
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js,> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    match self {
                        Enum::A(_0, _1) => {
                            let val = rquickjs::Array::new(ctx)?;
                            val.set(0, _0)?;
                            val.set(1, _1)?;
                            Ok(val.into_value())
                        }
                        Enum::B(_0) => {
                            let val = rquickjs::Array::new(ctx)?;
                            val.set(0, _0)?;
                            Ok(val.into_value())
                        }
                    }
                }
            }
        };

        enum_with_fields IntoJs {
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js,> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    match self {
                        Enum::A { x: _x, y: _y } => {
                            let val = rquickjs::Object::new(ctx)?;
                            val.set("x", _x)?;
                            val.set("y", _y)?;
                            Ok(val.into_value())
                        }
                        Enum::B { msg: _msg } => {
                            let val = rquickjs::Object::new(ctx)?;
                            val.set("msg", _msg)?;
                            Ok(val.into_value())
                        }
                    }
                }
            }
        };
    }
}
