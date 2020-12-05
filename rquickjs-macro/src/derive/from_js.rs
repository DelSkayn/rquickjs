use super::{data_fields, has_lifetime, new_lifetime, DataContent};
use crate::{abort, Config, Ident, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Index};

pub struct FromJs {
    config: Config,
}

impl FromJs {
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
                let mut iter = data.variants.iter();
                let first_fn = if let Some(variant) = iter.next() {
                    let body = self.from_fields(&ident, Some(&variant.ident), &variant.fields);

                    quote! {
                        {
                            let val = val.clone();
                            move || -> #lib_crate::Result<_> { #body }
                        }
                    }
                } else {
                    abort!(ident.span(), "Empty enum");
                };
                let rest_fns = iter.map(|variant| {
                    let body = self.from_fields(&ident, Some(&variant.ident), &variant.fields);

                    quote! {
                        {
                            let val = val.clone();
                            move |error| {
                                if error.is_from_js() {
                                    #body
                                } else {
                                    Err(error)
                                }
                            }
                        }
                    }
                });

                quote! {
                    (#first_fn)()
                    #(.or_else(#rest_fns))*
                }
            }
            _ => abort!(ident.span(), "Only structs and enums are supported"),
        };

        quote! {
            impl<#impl_params> #lib_crate::FromJs<#js_lt> for #typename #where_clause {
                fn from_js(ctx: #lib_crate::Ctx<#js_lt>, val: #lib_crate::Value<#js_lt>) -> #lib_crate::Result<Self> {
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
                quote! {
                    let val = val.into_object()?;
                    Ok(#ctor {
                        #(#fields: val.get(#field_names)?,)*
                    })
                }
            }
            Points(count) => {
                let point_indexes = (0..count).map(Index::from);
                quote! {
                    let val = val.into_array()?;
                    Ok(#ctor (
                        #(val.get(#point_indexes)?,)*
                    ))
                }
            }
            Nothing => {
                if let Some(variant) = variant {
                    let name = variant.to_string();
                    quote! {
                        let val = val.into_string()?.to_string()?;
                        if val == #name {
                            Ok(#ctor)
                        } else {
                            Err(#lib_crate::Error::new_from_js("string", #name))
                        }
                    }
                } else {
                    quote! {
                        Ok(#ctor)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        unit_struct FromJs {
            struct SomeStruct;
        } {
            impl<'js,> rquickjs::FromJs<'js> for SomeStruct {
                fn from_js(ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    Ok(SomeStruct)
                }
            }
        };

        tuple_struct FromJs {
            struct Struct(i32, String);
        } {
            impl<'js,> rquickjs::FromJs<'js> for Struct {
                fn from_js(ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let val = val.into_array()?;
                    Ok(Struct(
                        val.get(0)?,
                        val.get(1)?,
                    ))
                }
            }
        };

        struct_with_fields FromJs {
            struct Struct {
                int: i32,
                text: String,
            }
        } {
            impl<'js,> rquickjs::FromJs<'js> for Struct {
                fn from_js(ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let val = val.into_object()?;
                    Ok(Struct {
                        int: val.get("int")?,
                        text: val.get("text")?,
                    })
                }
            }
        };

        unit_enum FromJs {
            enum Enum {
                A,
                B,
            }
        } {
            impl<'js,> rquickjs::FromJs<'js> for Enum {
                fn from_js (ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    ({
                        let val = val.clone();
                        move || -> rquickjs::Result<_> {
                            let val = val.into_string()?.to_string()?;
                            if val == "A" {
                                Ok(Enum::A)
                            } else {
                                Err(rquickjs::Error::new_from_js("string", "A"))
                            }
                        }
                    })().or_else({
                        let val = val.clone();
                        move |error| {
                            if error.is_from_js() {
                                let val = val.into_string()?.to_string()?;
                                if val == "B" {
                                    Ok(Enum::B)
                                } else {
                                    Err(rquickjs::Error::new_from_js("string", "B"))
                                }
                            } else {
                                Err(error)
                            }
                        }
                    })
                }
            }
        };

        tuple_enum FromJs {
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js,> rquickjs::FromJs<'js> for Enum {
                fn from_js (ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    ({
                        let val = val.clone();
                        move || -> rquickjs::Result<_> {
                            let val = val.into_array()?;
                            Ok(Enum::A(
                                val.get(0)?,
                                val.get(1)?,
                            ))
                        }
                    })().or_else({
                        let val = val.clone();
                        move |error| {
                            if error.is_from_js() {
                                let val = val.into_array()?;
                                Ok(Enum::B(
                                    val.get(0)?,
                                ))
                            } else {
                                Err(error)
                            }
                        }
                    })
                }
            }
        };

        enum_with_fields FromJs {
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js,> rquickjs::FromJs<'js> for Enum {
                fn from_js (ctx: rquickjs::Ctx<'js>, val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    ({
                        let val = val.clone();
                        move || -> rquickjs::Result<_> {
                            let val = val.into_object()?;
                            Ok(Enum::A {
                                x: val.get("x")?,
                                y: val.get("y")?,
                            })
                        }
                    })().or_else({
                        let val = val.clone();
                        move |error| {
                            if error.is_from_js() {
                                let val = val.into_object()?;
                                Ok(Enum::B {
                                    msg: val.get("msg")?,
                                })
                            } else {
                                Err(error)
                            }
                        }
                    })
                }
            }
        };
    }
}
