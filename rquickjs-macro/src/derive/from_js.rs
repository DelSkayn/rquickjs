use super::{DataField, DataType, DataVariant, EnumRepr};
use crate::{Config, Ident, TokenStream};
use darling::ast::{Data, Fields, Style};
use quote::quote;
use std::collections::HashMap;
use syn::{parse_quote, Index};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SourceType {
    Int,
    String,
    Array,
    Object,
    Value,
    Void,
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
        let ident = &input.ident;
        let impl_params = input.impl_params(true);
        let type_name = input.type_name();
        let where_clause = input.where_clause(
            Some(parse_quote!(T: #lib_crate::FromJs<'js>)),
            Some(parse_quote!(T: Default)),
        );

        use Data::*;
        let body = match &input.data {
            Struct(fields) => {
                let (body, src) = self.expand_fields(input, ident, None, fields);
                src.wrap_value(lib_crate, body)
            }
            Enum(variants) => {
                use EnumRepr::*;

                let body = if input.enum_repr() == Untagged {
                    self.expand_variants_untagged(input, ident, variants)
                } else {
                    self.expand_variants_tagged(input, ident, variants)
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
                    Untagged => quote! { #body },
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

    fn expand_variants_tagged(
        &self,
        input: &DataType,
        ident: &Ident,
        variants: &[DataVariant],
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let variants = variants.iter().map(|variant| {
            let tag = input.name_for(variant).unwrap();
            let (body, src) =
                self.expand_fields(input, ident, Some(&variant.ident), &variant.fields);
            let body = src.wrap_value(lib_crate, body);
            quote! {
                #tag => { #body }
            }
        });

        quote! {
            match _tag.as_str() {
                #(#variants,)*
                _ => Err(#lib_crate::Error::new_from_js_message("value", "enum", "Unknown tag")),
            }
        }
    }

    fn expand_variants_untagged(
        &self,
        input: &DataType,
        ident: &Ident,
        variants: &[DataVariant],
    ) -> TokenStream {
        let variants = variants.iter().map(|variant| {
            if variant.fields.style == Style::Unit {
                let variant_ident = &variant.ident;
                let ctor = quote! { #ident::#variant_ident };
                if let Some(expr) = &variant.discriminant {
                    (quote! { #expr => Ok(#ctor), }, SourceType::Int)
                } else {
                    let name = input.name_for(variant).unwrap();
                    (quote! { #name => Ok(#ctor), }, SourceType::String)
                }
            } else {
                self.expand_fields(input, ident, Some(&variant.ident), &variant.fields)
            }
        });
        self.expand_variants(ident, variants)
    }

    fn expand_variants<I: Iterator<Item = (TokenStream, SourceType)>>(
        &self,
        ident: &Ident,
        variants: I,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
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
        ident: &Ident,
        variant: Option<&Ident>,
        fields: &Fields<DataField>,
    ) -> (TokenStream, SourceType) {
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
                    let value = if field.is_used() {
                        quote! { _val.get(#name)? }
                    } else {
                        field.default()
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
                        field.default()
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
        unit_struct FromJs {
            struct SomeStruct;
        } {
            impl<'js> rquickjs::FromJs<'js> for SomeStruct {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    Ok(SomeStruct)
                }
            }
        };

        newtype_struct FromJs {
            struct Newtype(i32);
        } {
            impl<'js> rquickjs::FromJs<'js> for Newtype {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    Ok(Newtype(_val.get()?))
                }
            }
        };

        tuple_struct FromJs {
            struct Struct(i32, String);
        } {
            impl<'js> rquickjs::FromJs<'js> for Struct {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let _val: rquickjs::Array = _val.get()?;
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
            impl<'js> rquickjs::FromJs<'js> for Struct {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let _val: rquickjs::Object = _val.get()?;
                    Ok(Struct {
                        int: _val.get("int")?,
                        text: _val.get("text")?,
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
            impl<'js> rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let (_tag, _val): (String, rquickjs::Value) = _val
                        .get::<rquickjs::Object>()?
                        .props()
                        .next()
                        .ok_or_else(|| rquickjs::Error::new_from_js_message("value", "enum", "Missing property"))??;
                    match _tag.as_str() {
                        "A" => {
                            Ok(Enum::A(_val.get()?))
                        },
                        "B" => {
                            let _val: rquickjs::Object = _val.get()?;
                            Ok(Enum::B {
                                s: _val.get("s")?,
                            })
                        },
                        "C" => {
                            Ok(Enum::C)
                        },
                        _ => Err(rquickjs::Error::new_from_js_message("value", "enum", "Unknown tag")),
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
            impl<'js> rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let _val: String = _val.get()?;
                    match _val.as_str() {
                        "A" => Ok(Enum::A),
                        "B" => Ok(Enum::B),
                        _ => Err(rquickjs::Error::new_from_js("string", "Enum")),
                    }
                }
            }
        };

        unit_enum_untagged_with_discriminant FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A = 1,
                B = 2,
            }
        } {
            impl<'js> rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let _val: i32 = _val.get()?;
                    match _val {
                        1 => Ok(Enum::A),
                        2 => Ok(Enum::B),
                        _ => Err(rquickjs::Error::new_from_js("int", "Enum")),
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
            impl<'js> rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    match _val.type_of() {
                        rquickjs::Type::Array => {
                            let _val: rquickjs::Array = _val.get()?;
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

        enum_with_fields_untagged FromJs {
            #[quickjs(untagged)]
            enum Enum {
                A { x: i8, y: i8 },
                B { msg: String },
            }
        } {
            impl<'js> rquickjs::FromJs<'js> for Enum {
                fn from_js(_ctx: rquickjs::Ctx<'js>, _val: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                    let _val: rquickjs::Object = _val.get()?;
                    (|| -> rquickjs::Result<_> {
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
    }
}
