use super::{DataField, DataType, DataVariant, EnumRepr};
use crate::{Config, Ident, TokenStream};
use darling::ast::{Data, Fields, Style};
use quote::quote;
use std::collections::HashMap;
use syn::{parse_quote, Index};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SourceType {
    Void,
    Int,
    String,
    Array,
    Object,
    Value,
}

impl SourceType {
    fn pattern(&self, lib_crate: &Ident) -> TokenStream {
        use SourceType::*;
        match self {
            Int => quote! { #lib_crate::Type::String::Int },
            String => quote! { #lib_crate::Type::String },
            Array => quote! { #lib_crate::Type::Array },
            Object => quote! { #lib_crate::Type::Object },
            Value => quote! { _ },
            Void => quote! {
                #lib_crate::Type::Uninitialized |
                #lib_crate::Type::Undefined |
                #lib_crate::Type::Null
            },
        }
    }

    fn value(&self, lib_crate: &Ident) -> TokenStream {
        use SourceType::*;
        match self {
            Int => quote! { let _val: i32 = _val.get()?; },
            String => quote! { let _val: String = _val.get()?; },
            Object => quote! { let _val: #lib_crate::Object = _val.get()?; },
            Array => quote! { let _val: #lib_crate::Array = _val.get()?; },
            _ => quote! {},
        }
    }

    fn wrap_value(&self, lib_crate: &Ident, value: TokenStream) -> TokenStream {
        let decl = self.value(lib_crate);
        if decl.is_empty() {
            value
        } else {
            quote! {
                #decl
                #value
            }
        }
    }
}

pub struct FromJs {
    config: Config,
}

impl FromJs {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn expand(&self, input: &DataType) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let impl_params = input.impl_params(true);
        let type_name = input.type_name();
        let where_clause = input.where_clause(
            Some(parse_quote!(T: #lib_crate::FromJs<'js>)),
            Some(parse_quote!(T: Default)),
        );

        use Data::*;
        let body = match &input.data {
            Struct(fields) => {
                let (body, src) = self.expand_fields(input, None, fields);
                src.wrap_value(lib_crate, body)
            }
            Enum(variants) => {
                use EnumRepr::*;

                let body = if let Untagged { constant } = input.enum_repr() {
                    if constant {
                        self.expand_variants_constant(input, variants)
                    } else {
                        self.expand_variants_untagged(input, variants)
                    }
                } else {
                    self.expand_variants_tagged(input, variants)
                };

                match input.enum_repr() {
                    ExternallyTagged => quote! {
                        let (_tag, _val): (String, #lib_crate::Value) = _val.get::<#lib_crate::Object>()?.props().next().ok_or_else(|| #lib_crate::Error::new_from_js_message("value", "enum", "Missing property"))??;
                        #body
                    },
                    InternallyTagged { tag } => quote! {
                        let _tag: String = _val.get::<#lib_crate::Object>()?.get(#tag)?;
                        #body
                    },
                    AdjacentlyTagged { tag, content } => quote! {
                        let _val: #lib_crate::Object = _val.get()?;
                        let _tag: String = _val.get(#tag)?;
                        let _val: #lib_crate::Value = _val.get(#content)?;
                        #body
                    },
                    Untagged { .. } => quote! { #body },
                }
            }
        };

        quote! {
            impl<#impl_params> #lib_crate::FromJs<'js> for #type_name #where_clause {
                fn from_js(_ctx: #lib_crate::Ctx<'js>, _val: #lib_crate::Value<'js>) -> #lib_crate::Result<Self> {
                    #body
                }
            }
        }
    }

    fn expand_variants_tagged(&self, input: &DataType, variants: &[DataVariant]) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let variants = variants.iter().map(|variant| {
            let tag = input.name_for(variant).unwrap();
            let (body, src) = self.expand_fields(input, Some(&variant.ident), &variant.fields);
            let body = src.wrap_value(lib_crate, body);
            quote! {
                #tag => { #body }
            }
        });

        quote! {
            match _tag.as_str() {
                #(#variants,)*
                tag => Err(#lib_crate::Error::new_from_js_message("value", "enum", format!("Unknown tag '{}'", tag))),
            }
        }
    }

    fn expand_variants_constant(&self, input: &DataType, variants: &[DataVariant]) -> TokenStream {
        let ident = &input.ident;
        let variants = variants.iter().map(|variant| {
            let variant_ident = &variant.ident;
            let ctor = quote! { #ident::#variant_ident };
            if let Some(expr) = &variant.discriminant {
                (quote! { #expr => Ok(#ctor), }, SourceType::Int)
            } else {
                let name = input.name_for(variant).unwrap();
                (quote! { #name => Ok(#ctor), }, SourceType::String)
            }
        });
        self.expand_variants(input, variants)
    }

    fn expand_variants_untagged(&self, input: &DataType, variants: &[DataVariant]) -> TokenStream {
        let ident = &input.ident;
        let variants = variants.iter().map(|variant| {
            if variant.fields.style == Style::Unit {
                let variant_ident = &variant.ident;
                let ctor = quote! { #ident::#variant_ident };
                (quote! { Ok(#ctor) }, SourceType::Void)
            } else {
                self.expand_fields(input, Some(&variant.ident), &variant.fields)
            }
        });
        self.expand_variants(input, variants)
    }

    fn expand_variants<I: Iterator<Item = (TokenStream, SourceType)>>(
        &self,
        input: &DataType,
        variants: I,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let ident = &input.ident;
        let mut grouped = HashMap::<_, Vec<_>>::new();

        for (body, src) in variants {
            grouped.entry(src).or_default().push(body);
        }

        let mut sources = grouped.keys().collect::<Vec<_>>();
        sources.sort();

        let multiple_sources = sources.len() > 1;

        let bodies = sources.into_iter().map(|src| {
            let variants = grouped.get(src).unwrap();
            let multiple_alternatives = variants.len() > 1;
            let variants = variants.iter();
            let body = if *src == SourceType::Int {
                let name = ident.to_string();
                quote! {
                    match _val {
                        #(#variants)*
                        _ => Err(#lib_crate::Error::new_from_js("int", #name)),
                    }
                }
            } else if *src == SourceType::String {
                let name = ident.to_string();
                quote! {
                    match _val.as_str() {
                        #(#variants)*
                        _ => Err(#lib_crate::Error::new_from_js("string", #name)),
                    }
                }
            } else {
                let variants = variants.enumerate().map(|(index, body)| {
                    if multiple_alternatives {
                        if index > 0 {
                            quote! {
                                .or_else(|error| if error.is_from_js() {
                                    #body
                                } else {
                                    Err(error)
                                })
                            }
                        } else {
                            quote! {
                                (|| -> #lib_crate::Result<_> {
                                    #body
                                })()
                            }
                        }
                    } else {
                        quote! { #body }
                    }
                });
                quote! { #(#variants)* }
            };
            let body = src.wrap_value(lib_crate, body);
            if multiple_sources {
                let pat = src.pattern(lib_crate);
                quote! { #pat => { #body } }
            } else {
                body
            }
        });

        if multiple_sources {
            quote! {
                match _val.type_of() {
                    #(#bodies)*
                }
            }
        } else {
            quote! {
                #(#bodies)*
            }
        }
    }

    fn expand_fields(
        &self,
        input: &DataType,
        variant: Option<&Ident>,
        fields: &Fields<DataField>,
    ) -> (TokenStream, SourceType) {
        let ident = &input.ident;
        let ctor = variant
            .map(|variant| quote! { #ident::#variant })
            .unwrap_or_else(|| quote! { #ident });

        use Style::*;
        match fields.style {
            Unit => (quote! { Ok(#ctor) }, SourceType::Void),
            Struct => {
                let fields = fields.fields.iter().map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let name = input.name_for(field).unwrap();
                    let default = field.default();
                    let value = if field.is_used() {
                        if field.has_default() {
                            quote! { _val.get::<_, Option<_>>(#name)?.unwrap_or_else(#default) }
                        } else {
                            quote! { _val.get(#name)? }
                        }
                    } else {
                        quote! { #default() }
                    };
                    quote! { #ident: #value }
                });

                (quote! { Ok(#ctor { #(#fields,)* }) }, SourceType::Object)
            }
            Tuple => {
                let count = fields.fields.iter().filter(|field| field.is_used()).count();
                let fields = fields.fields.iter().enumerate().map(|(index, field)| {
                    let value = if field.is_used() {
                        let index = Index::from(index);
                        quote! { _val.get(#index)? }
                    } else {
                        let default = field.default();
                        quote! { #default() }
                    };
                    quote! { #value }
                });

                if count == 1 {
                    (quote! { Ok(#ctor(_val.get()?)) }, SourceType::Value)
                } else {
                    (quote! { Ok(#ctor(#(#fields,)*)) }, SourceType::Array)
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        rquickjs,
        unit_struct FromJs {
            struct SomeStruct;
        } {
            impl<'js> #rquickjs::FromJs<'js> for SomeStruct {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    Ok(SomeStruct)
                }
            }
        };

        newtype_struct FromJs {
            struct Newtype(i32);
        } {
            impl<'js> #rquickjs::FromJs<'js> for Newtype {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    Ok(Newtype(_val.get()?))
                }
            }
        };

        newtype_struct_generic FromJs {
            struct Newtype<T>(T);
        } {
            impl<'js, T> #rquickjs::FromJs<'js> for Newtype<T>
            where
                T: #rquickjs::FromJs<'js>
            {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    Ok(Newtype(_val.get()?))
                }
            }
        };

        tuple_struct FromJs {
            struct Struct(i32, String);
        } {
            impl<'js> #rquickjs::FromJs<'js> for Struct {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Array = _val.get()?;
                    Ok(Struct(
                        _val.get(0)?,
                        _val.get(1)?,
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
            impl<'js> #rquickjs::FromJs<'js> for Struct {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Object = _val.get()?;
                    Ok(Struct {
                        int: _val.get("int")?,
                        text: _val.get("text")?,
                    })
                }
            }
        };

        struct_with_fields_generic FromJs {
            struct Struct<N, T> {
                int: N,
                text: T,
            }
        } {
            impl<'js, N, T> #rquickjs::FromJs<'js> for Struct<N, T>
            where
                T: #rquickjs::FromJs<'js>,
                N: #rquickjs::FromJs<'js>
            {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Object = _val.get()?;
                    Ok(Struct {
                        int: _val.get("int")?,
                        text: _val.get("text")?,
                    })
                }
            }
        };

        struct_with_fields_default FromJs {
            struct Struct {
                #[quickjs(default, skip_default)]
                int: i32,
                #[quickjs(default = "default_text", skip_default)]
                text: String,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Struct {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Object = _val.get()?;
                    Ok(Struct {
                        int: _val.get::<_, Option<_>>("int")?.unwrap_or_else(Default::default),
                        text: _val.get::<_, Option<_>>("text")?.unwrap_or_else(default_text),
                    })
                }
            }
        };

        enum_externally_tagged FromJs {
            enum Enum {
                A(f32),
                B { s: String },
                C,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let (_tag, _val): (String, #rquickjs::Value) = _val
                        .get::<#rquickjs::Object>()?
                        .props()
                        .next()
                        .ok_or_else(|| #rquickjs::Error::new_from_js_message("value", "enum", "Missing property"))??;
                    match _tag.as_str() {
                        "A" => {
                            Ok(Enum::A(_val.get()?))
                        },
                        "B" => {
                            let _val: #rquickjs::Object = _val.get()?;
                            Ok(Enum::B {
                                s: _val.get("s")?,
                            })
                        },
                        "C" => {
                            Ok(Enum::C)
                        },
                        tag => Err(#rquickjs::Error::new_from_js_message("value", "enum", format!("Unknown tag '{}'", tag))),
                    }
                }
            }
        };

        unit_enum_untagged FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A,
                B,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: String = _val.get()?;
                    match _val.as_str() {
                        "A" => Ok(Enum::A),
                        "B" => Ok(Enum::B),
                        _ => Err(#rquickjs::Error::new_from_js("string", "Enum")),
                    }
                }
            }
        };

        unit_enum_with_discriminant_untagged FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A = 1,
                B = 2,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: i32 = _val.get()?;
                    match _val {
                        1 => Ok(Enum::A),
                        2 => Ok(Enum::B),
                        _ => Err(#rquickjs::Error::new_from_js("int", "Enum")),
                    }
                }
            }
        };

        tuple_enum_untagged FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A(i8, i8),
                B(String),
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    match _val.type_of() {
                        #rquickjs::Type::Array => {
                            let _val: #rquickjs::Array = _val.get()?;
                            Ok(Enum::A(
                                _val.get(0)?,
                                _val.get(1)?,
                            ))
                        }
                        _ => {
                            Ok(Enum::B(_val.get()?))
                        }
                    }
                }
            }
        };

        enum_with_fields_externally_tagged FromJs {
            enum Enum {
                A { x: i8, #[quickjs(default)] y: i8 },
                B { msg: String },
                C,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let (_tag, _val): (String, #rquickjs::Value) = _val.get::<#rquickjs::Object>()?
                        .props().next()
                        .ok_or_else(|| #rquickjs::Error::new_from_js_message("value", "enum", "Missing property"))??;
                    match _tag.as_str() {
                        "A" => {
                            let _val: #rquickjs::Object = _val.get()?;
                            Ok(Enum::A {
                                x: _val.get("x")?,
                                y: _val.get::<_, Option<_>>("y")?.unwrap_or_else(Default::default),
                            })
                        },
                        "B" => {
                            let _val: #rquickjs::Object = _val.get()?;
                            Ok(Enum::B {
                                msg:_val.get("msg")?,
                            })
                        },
                        "C" => { Ok(Enum::C) },
                        tag => Err(#rquickjs::Error::new_from_js_message("value", "enum", format!("Unknown tag '{}'" , tag))),
                    }
                }
            }
        };

        enum_with_fields_internally_tagged FromJs {
            #[quickjs(tag = "$")]
            enum Enum {
                A { x: i8, y: i8 },
                B { #[quickjs(default = "default_msg")] msg: String },
                C,
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _tag: String = _val.get::<#rquickjs::Object>()?.get("$")?;
                    match _tag.as_str() {
                        "A" => {
                            let _val: #rquickjs::Object = _val.get()?;
                            Ok(Enum::A {
                                x: _val.get("x")?,
                                y: _val.get("y")?,
                            })
                        },
                        "B" => {
                            let _val: #rquickjs::Object = _val.get()?;
                            Ok(Enum::B {
                                msg: _val.get::<_, Option<_>>("msg")?.unwrap_or_else(default_msg),
                            })
                        },
                        "C" => {
                            Ok(Enum::C)
                        },
                        tag => Err(#rquickjs::Error::new_from_js_message("value", "enum", format!("Unknown tag '{}'" , tag))),
                    }
                }
            }
        };

        enum_with_fields_untagged FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js> #rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Object = _val.get()?;
                    (|| -> #rquickjs::Result<_> {
                        Ok(Enum::A {
                            x: _val.get("x")?,
                            y: _val.get("y")?,
                        })
                    })()
                        .or_else(|error| if error.is_from_js() {
                            Ok(Enum::B {
                                msg: _val.get("msg")?,
                            })
                        } else {
                            Err(error)
                        })
                }
            }
        };

        enum_with_fields_untagged_generic FromJs {
            #[quickjs(untagged)]
            enum Enum<N, T> {
                A { x: N, y: N },
                B { msg: T },
            }
        } {
            impl<'js, N, T> #rquickjs::FromJs<'js> for Enum<N, T>
            where
                T: #rquickjs::FromJs<'js>,
                N: #rquickjs::FromJs<'js>
            {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    let _val: #rquickjs::Object = _val.get()?;
                    (|| -> #rquickjs::Result<_> {
                        Ok(Enum::A {
                            x: _val.get("x")?,
                            y: _val.get("y")?,
                        })
                    })()
                        .or_else(|error| if error.is_from_js() {
                            Ok(Enum::B {
                                msg: _val.get("msg")?,
                            })
                        } else {
                            Err(error)
                        })
                }
            }
        };

        enum_with_value_and_unit_untagged FromJs {
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
            impl<'js> #rquickjs::FromJs<'js> for Any {
                fn from_js(_ctx: #rquickjs::Ctx<'js>, _val: #rquickjs::Value<'js>) -> #rquickjs::Result<Self> {
                    match _val.type_of() {
                        #rquickjs::Type::Uninitialized | #rquickjs::Type::Undefined | #rquickjs::Type::Null => { Ok(Any::None) }
                        _ => {
                            (|| -> #rquickjs::Result<_> {
                                Ok(Any::Bool(_val.get()?))
                            })().or_else(|error| if error.is_from_js() {
                                Ok(Any::Int(_val.get()?))
                            } else {
                                Err(error)
                            }).or_else(|error| if error.is_from_js() {
                                Ok(Any::Float(_val.get()?))
                            } else {
                                Err(error)
                            }).or_else(|error| if error.is_from_js() {
                                Ok(Any::Str(_val.get()?))
                            } else {
                                Err(error)
                            }).or_else(|error| if error.is_from_js() {
                                Ok (Any::List(_val.get()?))
                            } else {
                                Err(error)
                            }).or_else(|error| if error.is_from_js() {
                                Ok(Any::Dict(_val.get()?))
                            } else {
                                Err(error)
                            })
                        }
                    }
                }
            }
        };
    }
}
