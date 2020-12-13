use super::{DataField, DataType, DataVariant, EnumRepr};
use crate::{Config, Ident, TokenStream};
use darling::ast::{Data, Fields, Style};
use quote::{format_ident, quote};
use syn::{parse_quote, Index};

pub struct IntoJs {
    config: Config,
}

impl IntoJs {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn expand(&self, input: &DataType) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let ident = &input.ident;
        let impl_params = input.impl_params(true);
        let type_name = input.type_name();
        let where_clause = input.where_clause(
            Some(parse_quote!(T: #lib_crate::IntoJs<'js>)),
            Some(parse_quote!(T: Default)),
        );

        use Data::*;
        let body = match &input.data {
            Struct(fields) => self.expand_struct_fields(input, &fields),
            Enum(variants) => {
                let bodies = variants
                    .iter()
                    .map(|variant| self.expand_enum_fields(input, ident, variant));

                let body = quote! {
                    match self {
                        #(#bodies)*
                    }
                };

                use EnumRepr::*;
                match input.enum_repr() {
                    ExternallyTagged => quote! {
                        let (_tag, _data) = #body;
                        let _val = #lib_crate::Object::new(_ctx)?;
                        _val.set(_tag, _data)?;
                        Ok(_val.into_value())
                    },
                    InternallyTagged { tag } => quote! {
                        let (_tag, _val) = #body;
                        _val.as_object().unwrap().set(#tag, _tag)?;
                        Ok(_val)
                    },
                    AdjacentlyTagged { tag, content } => quote! {
                        let (_tag, _data) = #body;
                        let _val = #lib_crate::Object::new(_ctx)?;
                        _val.set(#tag, _tag)?;
                        _val.set(#content, _data)?;
                        Ok(_val.into_value())
                    },
                    Untagged => quote! { Ok(#body) },
                }
            }
        };

        quote! {
            impl<#impl_params> #lib_crate::IntoJs<'js> for #type_name #where_clause {
                fn into_js(self, _ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>> {
                    #body
                }
            }
        }
    }

    fn expand_struct_fields(&self, input: &DataType, fields: &Fields<DataField>) -> TokenStream {
        let lib_crate = &self.config.lib_crate;

        use Style::*;
        match fields.style {
            Unit => quote! { Ok(#lib_crate::Value::new_undefined(_ctx)) },
            Struct => {
                let field_names = fields
                    .fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| input.name_for(field).unwrap());

                let field_idents = fields
                    .fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| field.ident.as_ref().unwrap());

                quote! {
                    let _val = #lib_crate::Object::new(_ctx)?;
                    #(_val.set(#field_names, self.#field_idents)?;)*
                    Ok(_val.into_value())
                }
            }
            Tuple => {
                let array_indexes = fields
                    .fields
                    .iter()
                    .filter(|field| field.is_used())
                    .enumerate()
                    .map(|(index, _)| Index::from(index))
                    .collect::<Vec<_>>();

                let field_indexes = fields
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.is_used())
                    .map(|(index, _)| Index::from(index));

                if array_indexes.len() > 1 {
                    quote! {
                        let _val = #lib_crate::Array::new(_ctx)?;
                        #(_val.set(#array_indexes, self.#field_indexes)?;)*
                        Ok(_val.into_value())
                    }
                } else if !array_indexes.is_empty() {
                    quote! { #(self.#field_indexes.into_js(_ctx)?)* }
                } else {
                    quote! { Ok(#lib_crate::Value::new_undefined(_ctx)) }
                }
            }
        }
    }

    fn expand_enum_fields(
        &self,
        input: &DataType,
        ident: &Ident,
        variant: &DataVariant,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let variant_ident = &variant.ident;
        let ctor = quote! { #ident::#variant_ident };
        let name = input.name_for(variant).unwrap();
        let enum_repr = input.enum_repr();

        use EnumRepr::*;
        use Style::*;

        let unit = {
            match enum_repr {
                Untagged => {
                    if let Some(expr) = &variant.discriminant {
                        quote! { #expr.into_js(_ctx)? }
                    } else {
                        quote! { #name.into_js(_ctx)? }
                    }
                }
                InternallyTagged { .. } => quote! { #lib_crate::Object::new(_ctx)? },
                _ => quote! { #lib_crate::Value::new_undefined(_ctx) },
            }
        };

        let style = variant.fields.style;
        let fields = &variant.fields.fields;

        let pattern = match style {
            Unit => quote! { #ctor },
            Struct => {
                let field_idents = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| field.ident.as_ref().unwrap());

                let field_aliases = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| format_ident!("__{}", field.ident.as_ref().unwrap()))
                    .collect::<Vec<_>>();

                let rest = if field_aliases.len() < fields.len() {
                    quote! { .. }
                } else {
                    quote! {}
                };

                quote! { #ctor { #(#field_idents: #field_aliases),* #rest } }
            }
            Tuple => {
                let field_patterns = fields
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format_ident!("_{}", index));

                quote! { #ctor(#(#field_patterns),*) }
            }
        };

        let body = match style {
            Unit => quote! { #unit },
            Struct => {
                let field_names = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| input.name_for(field).unwrap());

                let field_aliases = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .map(|field| format_ident!("__{}", field.ident.as_ref().unwrap()))
                    .collect::<Vec<_>>();

                quote! {
                    {
                        let _val = #lib_crate::Object::new(_ctx)?;
                        #(_val.set(#field_names, #field_aliases)?;)*
                        _val.into_value()
                    }
                }
            }
            Tuple => {
                let array_indexes = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .enumerate()
                    .map(|(index, _)| Index::from(index))
                    .collect::<Vec<_>>();

                let field_aliases = fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.is_used())
                    .map(|(index, _)| format_ident!("_{}", index));

                if array_indexes.len() > 1 {
                    quote! {
                        {
                            let _val = #lib_crate::Array::new(_ctx)?;
                            #(_val.set(#array_indexes, #field_aliases)?;)*
                            _val.into_value()
                        }
                    }
                } else if !array_indexes.is_empty() {
                    quote! { #(#field_aliases.into_js(_ctx)?)* }
                } else {
                    unit
                }
            }
        };

        if enum_repr == Untagged {
            quote! { #pattern => #body, }
        } else {
            quote! { #pattern => (#name, #body), }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        unit_struct IntoJs {
            struct SomeStruct;
        } {
            impl<'js> rquickjs::IntoJs<'js> for SomeStruct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(rquickjs::Value::new_undefined(_ctx))
                }
            }
        };

        tuple_struct IntoJs {
            struct Struct(i32, String);
        } {
            impl<'js> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let _val = rquickjs::Array::new(_ctx)?;
                    _val.set(0, self.0)?;
                    _val.set(1, self.1)?;
                    Ok(_val.into_value())
                }
            }
        };

        struct_with_fields IntoJs {
            struct Struct {
                int: i32,
                text: String,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set("int", self.int)?;
                    _val.set("text", self.text)?;
                    Ok(_val.into_value())
                }
            }
        };

        untagged_unit_enum IntoJs {
            #[quickjs(untagged)]
            enum Enum {
                A,
                B,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(match self {
                        Enum::A => "A".into_js(_ctx)?,
                        Enum::B => "B".into_js(_ctx)?,
                    })
                }
            }
        };

        untagged_unit_enum_with_discriminant IntoJs {
            #[quickjs(untagged)]
            enum Enum {
                A = 1,
                B = 2,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(match self {
                        Enum::A => 1.into_js(_ctx)?,
                        Enum::B => 2.into_js(_ctx)?,
                    })
                }
            }
        };

        externally_tagged_tuple_enum IntoJs {
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _data) = match self {
                        Enum::A(_0, _1) => ("A", {
                            let _val = rquickjs::Array::new(_ctx)?;
                            _val.set(0, _0)?;
                            _val.set(1, _1)?;
                            _val.into_value()
                        }),
                        Enum::B(_0) => ("B", _0.into_js(_ctx)?),
                    };
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set(_tag, _data)?;
                    Ok(_val.into_value())
                }
            }
        };

        adjacently_tagged_tuple_enum IntoJs {
            #[quickjs(tag, content)]
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _data) = match self {
                        Enum::A(_0, _1) => ("A", {
                            let _val = rquickjs::Array::new(_ctx)?;
                            _val.set(0, _0)?;
                            _val.set(1, _1)?;
                            _val.into_value()
                        }),
                        Enum::B(_0) => ("B", _0.into_js(_ctx)?),
                    };
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set("tag", _tag)?;
                    _val.set("content", _data)?;
                    Ok(_val.into_value())
                }
            }
        };

        untagged_tuple_enum IntoJs {
            #[quickjs(untagged)]
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(match self {
                        Enum::A(_0, _1) => {
                            let _val = rquickjs::Array::new(_ctx)?;
                            _val.set(0, _0)?;
                            _val.set(1, _1)?;
                            _val.into_value()
                        },
                        Enum::B(_0) => _0.into_js(_ctx)?,
                    })
                }
            }
        };

        externally_tagged_enum_with_fields IntoJs {
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _data) = match self {
                        Enum::A { x: __x, y: __y } => ("A", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("x", __x)?;
                            _val.set("y", __y)?;
                            _val.into_value()
                        }),
                        Enum::B { msg: __msg } => ("B", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("msg", __msg)?;
                            _val.into_value()
                        }),
                    };
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set(_tag, _data)?;
                    Ok(_val.into_value())
                }
            }
        };

        internally_tagged_enum_with_fields IntoJs {
            #[quickjs(tag = "$")]
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _val) = match self {
                        Enum::A { x: __x, y: __y } => ("A", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("x", __x)?;
                            _val.set("y", __y)?;
                            _val.into_value()
                        }),
                        Enum::B { msg: __msg } => ("B", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("msg", __msg)?;
                            _val.into_value()
                        }),
                    };
                    _val.as_object().unwrap().set("$", _tag)?;
                    Ok(_val)
                }
            }
        };

        untagged_enum_with_fields IntoJs {
            #[quickjs(untagged)]
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(match self {
                        Enum::A { x: __x, y: __y } => {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("x", __x)?;
                            _val.set("y", __y)?;
                            _val.into_value()
                        },
                        Enum::B { msg: __msg } => {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("msg", __msg)?;
                            _val.into_value()
                        },
                    })
                }
            }
        };
    }
}
