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
                    Untagged { .. } => quote! { Ok(#body) },
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
                let assignments =
                    fields
                        .fields
                        .iter()
                        .filter(|field| field.is_used())
                        .map(|field| {
                            let name = input.name_for(field).unwrap();
                            let ident = field.ident.as_ref().unwrap();
                            if field.skip_default {
                                let default = field.default();
                                quote! {
                                    if PartialEq::ne(&self.#ident, &#default()) {
                                        _val.set(#name, self.#ident)?;
                                    }
                                }
                            } else {
                                quote! { _val.set(#name, self.#ident)?; }
                            }
                        });

                quote! {
                    let _val = #lib_crate::Object::new(_ctx)?;
                    #(#assignments)*
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
                Untagged { constant } => {
                    if constant {
                        if let Some(expr) = &variant.discriminant {
                            quote! { #expr.into_js(_ctx)? }
                        } else {
                            quote! { #name.into_js(_ctx)? }
                        }
                    } else {
                        quote! { #lib_crate::Undefined.into_js(_ctx)? }
                    }
                }
                InternallyTagged { .. } => quote! { #lib_crate::Object::new(_ctx)?.into_value() },
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
                let assignments = fields.iter().filter(|field| field.is_used()).map(|field| {
                    let name = input.name_for(field).unwrap();
                    let alias = format_ident!("__{}", field.ident.as_ref().unwrap());
                    if field.skip_default {
                        let default = field.default();
                        quote! {
                            if PartialEq::ne(&#alias, &#default()) {
                                _val.set(#name, #alias)?;
                            }
                        }
                    } else {
                        quote! { _val.set(#name, #alias)?; }
                    }
                });

                quote! {
                    {
                        let _val = #lib_crate::Object::new(_ctx)?;
                        #(#assignments)*
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

        if matches!(enum_repr, Untagged {..}) {
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

        struct_with_fields_default IntoJs {
            struct Struct {
                #[quickjs(default, skip_default)]
                int: i32,
                #[quickjs(default = "default_text", skip_default)]
                text: String,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let _val = rquickjs::Object::new(_ctx)?;
                    if PartialEq::ne(&self.int, &Default::default()) {
                        _val.set("int", self.int)?;
                    }
                    if PartialEq::ne(&self.text, &default_text()) {
                        _val.set("text", self.text)?;
                    }
                    Ok(_val.into_value())
                }
            }
        };

        enum_externally_tagged IntoJs {
            enum Enum {
                A(f32),
                B { s: String },
                C,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _data) = match self {
                        Enum::A(_0) => ("A", _0.into_js(_ctx)?),
                        Enum::B { s: __s } => ("B", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("s", __s)?;
                            _val.into_value()
                        }),
                        Enum::C => ("C", rquickjs::Value::new_undefined(_ctx)),
                    };
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set(_tag, _data)?;
                    Ok(_val.into_value())
                }
            }
        };

        unit_enum_untagged IntoJs {
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

        unit_enum_with_discriminant_untagged IntoJs {
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

        tuple_enum_externally_tagged IntoJs {
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

        tuple_enum_adjacently_tagged IntoJs {
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

        tuple_enum_untagged IntoJs {
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

        enum_with_fields_externally_tagged IntoJs {
            enum Enum {
                A { x: i8, #[quickjs(skip_default)] y: i8 },
                B { msg: String },
                C,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Enum {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let (_tag, _data) = match self {
                        Enum::A { x: __x, y: __y } => ("A", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("x", __x)?;
                            if PartialEq::ne(& __y, &Default::default()) {
                                _val.set("y", __y)?;
                            }
                            _val.into_value()
                        }),
                        Enum::B { msg: __msg } => ("B", {
                            let _val = rquickjs::Object::new(_ctx)?;
                            _val.set("msg", __msg)?;
                            _val.into_value()
                        }),
                        Enum::C => ("C", rquickjs::Value::new_undefined(_ctx)),
                    };
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set(_tag, _data)?;
                    Ok(_val.into_value())
                }
            }
        };

        enum_with_fields_internally_tagged IntoJs {
            #[quickjs(tag = "$")]
            enum Enum {
                A { x: i8, y: i8 },
                B { #[quickjs(default = "default_msg", skip_default)] msg: String },
                C,
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
                            if PartialEq::ne(&__msg, &default_msg()) {
                                _val.set("msg", __msg)?;
                            }
                            _val.into_value()
                        }),
                        Enum::C => ("C", rquickjs::Object::new(_ctx)?.into_value()),
                    };
                    _val.as_object().unwrap().set("$", _tag)?;
                    Ok(_val)
                }
            }
        };

        enum_with_fields_untagged IntoJs {
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

        enum_with_value_and_unit_untagged IntoJs {
            #[quickjs(untagged)]
            enum Any {
                None,
                Bool(bool),
                Int(i64),
                Float(f64),
                Str(String),
                List(Vec<Value>),
                Dict(Map<String, Value>),
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for Any {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(match self {
                        Any::None => rquickjs::Undefined.into_js(_ctx)?,
                        Any::Bool(_0) => _0.into_js(_ctx)?,
                        Any::Int(_0) => _0.into_js(_ctx)?,
                        Any::Float(_0) => _0.into_js(_ctx)?,
                        Any::Str(_0) => _0.into_js(_ctx)?,
                        Any::List(_0) => _0.into_js(_ctx)?,
                        Any::Dict(_0) => _0.into_js(_ctx)?,
                    })
                }
            }
        };
    }
}
