use super::{DataField, DataType, DataVariant, EnumRepr};
use crate::{Config, TokenStream};
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

    pub fn expand(&self, input: &DataType, byref: bool) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let impl_params = input.impl_params(true);
        let type_name = input.type_name();
        let (ref_def, ref_by, ref_of) = if byref {
            (quote! { for<'r> &'r }, quote! { & }, quote! {})
        } else {
            (quote! {}, quote! {}, quote! { & })
        };
        let where_clause = input.where_clause(
            Some(parse_quote!(#ref_def T: #lib_crate::IntoJs<'js>)),
            Some(parse_quote!(T: Default)),
        );

        use Data::*;
        let body = match &input.data {
            Struct(fields) => self.expand_struct_fields(input, &fields, &ref_of),
            Enum(variants) => {
                let bodies = variants
                    .iter()
                    .map(|variant| self.expand_enum_fields(input, variant, &ref_of));

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
            impl<#impl_params> #lib_crate::IntoJs<'js> for #ref_by #type_name #where_clause {
                fn into_js(self, _ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>> {
                    #body
                }
            }
        }
    }

    fn expand_struct_fields(
        &self,
        input: &DataType,
        fields: &Fields<DataField>,
        ref_of: &TokenStream,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let ctor = &input.ident;
        let style = &fields.style;
        let fields = &fields.fields;

        use Style::*;
        match style {
            Unit => quote! { Ok(#lib_crate::Value::new_undefined(_ctx)) },
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
                    quote! { , .. }
                } else {
                    quote! {}
                };

                let assignments = fields.iter().filter(|field| field.is_used()).map(|field| {
                    let name = input.name_for(field).unwrap();
                    let alias = format_ident!("__{}", field.ident.as_ref().unwrap());
                    if field.skip_default {
                        let default = field.default();
                        quote! {
                            if PartialEq::ne(#ref_of #alias, &#default()) {
                                _val.set(#name, #alias)?;
                            }
                        }
                    } else {
                        quote! { _val.set(#name, #alias)?; }
                    }
                });

                quote! {
                    let #ctor { #(#field_idents: #field_aliases),* #rest } = self;
                    let _val = #lib_crate::Object::new(_ctx)?;
                    #(#assignments)*
                    Ok(_val.into_value())
                }
            }
            Tuple => {
                let array_indexes = fields
                    .iter()
                    .filter(|field| field.is_used())
                    .enumerate()
                    .map(|(index, _)| Index::from(index))
                    .collect::<Vec<_>>();

                let field_patterns = fields
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format_ident!("_{}", index));

                let field_aliases = fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.is_used())
                    .map(|(index, _)| format_ident!("_{}", index));

                if array_indexes.len() > 1 {
                    quote! {
                        let #ctor(#(#field_patterns),*) = self;
                        let _val = #lib_crate::Array::new(_ctx)?;
                        #(_val.set(#array_indexes, #field_aliases)?;)*
                        Ok(_val.into_value())
                    }
                } else if !array_indexes.is_empty() {
                    quote! {
                        let #ctor(#(#field_patterns),*) = self;
                        #(#lib_crate::IntoJs::into_js(#field_aliases, _ctx))*
                    }
                } else {
                    quote! { Ok(#lib_crate::Value::new_undefined(_ctx)) }
                }
            }
        }
    }

    fn expand_enum_fields(
        &self,
        input: &DataType,
        variant: &DataVariant,
        ref_of: &TokenStream,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let ident = &input.ident;
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
                            quote! { #lib_crate::IntoJs::into_js(#expr, _ctx)? }
                        } else {
                            quote! { #lib_crate::IntoJs::into_js(#name, _ctx)? }
                        }
                    } else {
                        quote! { #lib_crate::Value::new_undefined(_ctx) }
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
                    quote! { , .. }
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
                            if PartialEq::ne(#ref_of #alias, &#default()) {
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
                    quote! { #(#lib_crate::IntoJs::into_js(#field_aliases, _ctx)?)* }
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

        unit_struct_byref IntoJsByRef {
            struct SomeStruct;
        } {
            impl<'js> rquickjs::IntoJs<'js> for &SomeStruct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    Ok(rquickjs::Value::new_undefined(_ctx))
                }
            }
        };

        newtype_struct IntoJs {
            struct Newtype(i32);
        } {
            impl<'js> rquickjs::IntoJs<'js> for Newtype {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Newtype(_0) = self;
                    rquickjs::IntoJs::into_js(_0, _ctx)
                }
            }
        };

        newtype_struct_byref IntoJsByRef {
            struct Newtype(i32);
        } {
            impl<'js> rquickjs::IntoJs<'js> for &Newtype {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Newtype(_0) = self;
                    rquickjs::IntoJs::into_js(_0, _ctx)
                }
            }
        };

        newtype_struct_generic IntoJs {
            struct Newtype<T>(T);
        } {
            impl<'js, T> rquickjs::IntoJs<'js> for Newtype<T>
            where
                T: rquickjs::IntoJs<'js>
            {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Newtype(_0) = self;
                    rquickjs::IntoJs::into_js(_0, _ctx)
                }
            }
        };

        newtype_struct_generic_byref IntoJsByRef {
            struct Newtype<T>(T);
        } {
            impl<'js, T> rquickjs::IntoJs<'js> for &Newtype<T>
            where
                for<'r> &'r T: rquickjs::IntoJs<'js>
            {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Newtype(_0) = self;
                    rquickjs::IntoJs::into_js(_0, _ctx)
                }
            }
        };

        tuple_struct IntoJs {
            struct Struct(i32, String);
        } {
            impl<'js> rquickjs::IntoJs<'js> for Struct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Struct(_0, _1) = self;
                    let _val = rquickjs::Array::new(_ctx)?;
                    _val.set(0, _0)?;
                    _val.set(1, _1)?;
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
                    let Struct { int: __int, text: __text } = self;
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set("int", __int)?;
                    _val.set("text", __text)?;
                    Ok(_val.into_value())
                }
            }
        };

        struct_with_fields_generic IntoJs {
            struct Struct<N, T> {
                int: N,
                text: T,
            }
        } {
            impl<'js, N, T> rquickjs::IntoJs<'js> for Struct<N, T>
            where
                T: rquickjs::IntoJs<'js>,
                N: rquickjs::IntoJs<'js>
            {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Struct { int: __int, text: __text } = self;
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set("int", __int)?;
                    _val.set("text", __text)?;
                    Ok(_val.into_value())
                }
            }
        };

        struct_with_fields_generic_byref IntoJsByRef {
            struct Struct<N, T> {
                int: N,
                text: T,
            }
        } {
            impl<'js, N, T> rquickjs::IntoJs<'js> for &Struct<N, T>
            where
                for<'r> &'r T: rquickjs::IntoJs<'js>,
                for<'r> &'r N: rquickjs::IntoJs<'js>
            {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Struct { int: __int, text: __text } = self;
                    let _val = rquickjs::Object::new(_ctx)?;
                    _val.set("int", __int)?;
                    _val.set("text", __text)?;
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
                    let Struct { int: __int, text: __text } = self;
                    let _val = rquickjs::Object::new(_ctx)?;
                    if PartialEq::ne(&__int, &Default::default()) {
                        _val.set("int", __int)?;
                    }
                    if PartialEq::ne(&__text, &default_text()) {
                        _val.set("text", __text)?;
                    }
                    Ok(_val.into_value())
                }
            }
        };

        struct_with_fields_default_byref IntoJsByRef {
            struct Struct {
                #[quickjs(default, skip_default)]
                int: i32,
                #[quickjs(default = "default_text", skip_default)]
                text: String,
            }
        } {
            impl<'js> rquickjs::IntoJs<'js> for &Struct {
                fn into_js(self, _ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                    let Struct { int: __int, text: __text } = self;
                    let _val = rquickjs::Object::new(_ctx)?;
                    if PartialEq::ne(__int, &Default::default()) {
                        _val.set("int", __int)?;
                    }
                    if PartialEq::ne(__text, &default_text()) {
                        _val.set("text", __text)?;
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
                        Enum::A(_0) => ("A", rquickjs::IntoJs::into_js(_0, _ctx)?),
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
                        Enum::A => rquickjs::IntoJs::into_js("A", _ctx)?,
                        Enum::B => rquickjs::IntoJs::into_js("B", _ctx)?,
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
                        Enum::A => rquickjs::IntoJs::into_js(1, _ctx)?,
                        Enum::B => rquickjs::IntoJs::into_js(2, _ctx)?,
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
                        Enum::B(_0) => ("B", rquickjs::IntoJs::into_js(_0, _ctx)?),
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
                        Enum::B(_0) => ("B", rquickjs::IntoJs::into_js(_0, _ctx)?),
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
                        Enum::B(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
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

        enum_with_fields_untagged_generic IntoJs {
            #[quickjs(untagged)]
            enum Enum<N, T> {
                A { x: N, y: N },
                B { msg: T },
            }
        } {
            impl<'js, N, T> rquickjs::IntoJs<'js> for Enum<N, T>
            where
                T: rquickjs::IntoJs<'js>,
                N: rquickjs::IntoJs<'js>
            {
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

        enum_with_fields_untagged_generic_byref IntoJsByRef {
            #[quickjs(untagged)]
            enum Enum<N, T> {
                A { x: N, y: N },
                B { msg: T },
            }
        } {
            impl<'js, N, T> rquickjs::IntoJs<'js> for &Enum<N, T>
            where
                for<'r> &'r T: rquickjs::IntoJs<'js>,
                for<'r> &'r N: rquickjs::IntoJs<'js>
            {
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
                        Any::None => rquickjs::Value::new_undefined(_ctx),
                        Any::Bool(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                        Any::Int(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                        Any::Float(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                        Any::Str(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                        Any::List(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                        Any::Dict(_0) => rquickjs::IntoJs::into_js(_0, _ctx)?,
                    })
                }
            }
        };
    }
}
