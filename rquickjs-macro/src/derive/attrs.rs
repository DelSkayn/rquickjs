use crate::{Config, Ident, TokenStream};
use darling::{
    ast::{Data, Fields, GenericParam, GenericParamExt, Generics, Style},
    usage::{CollectTypeParams, GenericsExt, Purpose},
    uses_lifetimes, uses_type_params,
    util::Override,
    FromDeriveInput, FromField, FromTypeParam, FromVariant,
};
use fnv::FnvHashSet;
use ident_case::RenameRule;
use quote::{quote, ToTokens};
use std::borrow::Cow;
use syn::{
    ConstParam, Expr, Lifetime, LifetimeDef, Path, Type, TypeParamBound, Visibility, WherePredicate,
};

pub type IdentSet = FnvHashSet<Ident>;
pub type LifetimeSet = FnvHashSet<Lifetime>;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(quickjs), supports(struct_any, enum_any))]
pub struct DataType {
    /// The data type ident
    pub ident: Ident,

    /// The data type generics
    pub generics: Generics<GenericParam<DataParam, LifetimeDef, ConstParam>>,

    /// The data contents
    pub data: Data<DataVariant, DataField>,

    /// Rename all the fields
    #[darling(default)]
    pub rename_all: RenameRule,

    /// Override inferred type parameters bounds
    #[darling(default)]
    pub bound: Option<Vec<WherePredicate>>,

    /// The tag field name for enum variant
    #[darling(default)]
    pub tag: Option<Override<String>>,

    /// The content field name for enum variant
    #[darling(default)]
    pub content: Option<Override<String>>,

    /// Do not use tags for enums
    #[darling(default)]
    pub untagged: bool,

    /// The name of library crate (usually `rquickjs`)
    #[darling(default, rename = "crate")]
    pub crate_: Option<Ident>,
}

/// The representation of enums
#[derive(Debug, PartialEq, Eq)]
pub enum EnumRepr<'a> {
    ExternallyTagged,
    InternallyTagged {
        tag: Cow<'a, str>,
    },
    AdjacentlyTagged {
        tag: Cow<'a, str>,
        content: Cow<'a, str>,
    },
    Untagged {
        constant: bool,
    },
}

/// Tha pattern to apply to type parameters in where clause
/// `for<$lifetimes> T: $bounds`
pub struct TypePredicatePattern {
    pub lifetimes: Vec<LifetimeDef>,
    pub bounds: Vec<TypeParamBound>,
}

impl TypePredicatePattern {
    pub fn apply(&self, type_param: &Ident) -> TokenStream {
        let lifetimes = &self.lifetimes;
        let bounds = &self.bounds;

        if lifetimes.is_empty() {
            quote! { #type_param: #(#bounds)+* }
        } else {
            quote! { for<#(#lifetimes),*> #type_param: #(#bounds)+* }
        }
    }
}

impl syn::parse::Parse for TypePredicatePattern {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        use WherePredicate::*;
        match WherePredicate::parse(input)? {
            Type(pt) => Ok(Self {
                lifetimes: pt
                    .lifetimes
                    .map(|bound| bound.lifetimes.into_iter().collect())
                    .unwrap_or_else(Vec::new),
                bounds: pt.bounds.into_iter().collect(),
            }),
            Lifetime(lt) => Err(syn::Error::new(
                lt.lifetime.apostrophe,
                "Type predicate required",
            )),
            Eq(eq) => Err(syn::Error::new(
                eq.eq_token.spans[0],
                "Type predicate required",
            )),
        }
    }
}

/// The helper trait for elements which can has name
/// (fields and enum variants)
pub trait Named {
    // Get original identity
    fn ident(&self, rule: &RenameRule) -> Option<Cow<'_, str>>;
    // Get new assigned name (if renamed)
    fn name(&self) -> Option<Cow<'_, str>>;
}

impl DataType {
    pub fn config(&self) -> Config {
        let mut config = Config::new();

        if let Some(crate_) = &self.crate_ {
            config.lib_crate = crate_.clone();
        }

        config
    }

    /// List of parameters with bounds for implimentation
    /// `impl<...>`
    pub fn impl_params(&self, need_js_lt: bool) -> TokenStream {
        let params = &self.generics.params;

        let has_js_lt = params.iter().any(|param| {
            if let GenericParam::Lifetime(lt) = param {
                lt.lifetime.ident == "js"
            } else {
                false
            }
        });

        let params = params.iter().map(|param| match param {
            GenericParam::Type(dp) => quote!(#dp),
            GenericParam::Lifetime(ld) => quote!(#ld),
            GenericParam::Const(cp) => quote!(#cp),
        });

        if need_js_lt && !has_js_lt {
            let params = std::iter::once(quote!('js)).chain(params);
            quote! { #(#params),* }
        } else {
            quote! { #(#params),* }
        }
    }

    /// Full name of type with parameters
    /// `Type<'a, T, ...>`
    pub fn type_name(&self) -> TokenStream {
        let ident = &self.ident;
        let params = self
            .generics
            .params
            .iter()
            .map(|param| match param {
                GenericParam::Type(DataParam { ident, .. }) => quote!(#ident),
                GenericParam::Lifetime(LifetimeDef { lifetime, .. }) => quote!(#lifetime),
                GenericParam::Const(c) => quote!(#c),
            })
            .collect::<Vec<_>>();

        if params.is_empty() {
            quote! { #ident }
        } else {
            quote! { #ident<#(#params),*> }
        }
    }

    /// The where clause for implementation
    /// `where ...`
    ///
    /// - `need_traits` is a type params traits for used fields, typically `FromJs`/`IntoJs`
    /// - `skip_traits` is a type params traits for skipped fields, typically `Default` for `FromJs`
    pub fn where_clause(
        &self,
        need_bound: Option<TypePredicatePattern>,
        skip_bound: Option<TypePredicatePattern>,
    ) -> TokenStream {
        let mut bounds = Vec::new();

        if let Some(where_clause) = &self.generics.where_clause {
            bounds.extend(
                where_clause
                    .predicates
                    .iter()
                    .map(|predicate| quote! { #predicate }),
            )
        }

        if let Some(bound) = &self.bound {
            bounds.extend(bound.iter().map(|predicate| quote! { #predicate }))
        } else {
            // imply predicates automatically
            let declared_type_params = self.declared_type_params();

            let options = Purpose::BoundImpl.into();

            use Data::*;
            match &self.data {
                Struct(data) => {
                    let fields = &data.fields;

                    if let Some(bound) = need_bound {
                        let type_params = fields
                            .iter()
                            .filter(|field| field.is_used())
                            .collect_type_params(&options, &declared_type_params);
                        bounds.extend(type_params.into_iter().map(|ident| bound.apply(ident)));
                    }
                    if let Some(bound) = skip_bound {
                        let type_params = fields
                            .iter()
                            .filter(|field| field.is_default())
                            .collect_type_params(&options, &declared_type_params);
                        bounds.extend(type_params.into_iter().map(|ident| bound.apply(ident)));
                    }
                }
                Enum(variants) => {
                    if let Some(bound) = need_bound {
                        let type_params = variants
                            .iter()
                            .filter(|variant| variant.is_used())
                            .flat_map(|variant| {
                                variant.fields.iter().filter(|field| field.is_used())
                            })
                            .collect_type_params(&options, &declared_type_params);
                        bounds.extend(type_params.into_iter().map(|ident| bound.apply(ident)));
                    }
                    if let Some(bound) = skip_bound {
                        let type_params = variants
                            .iter()
                            .filter(|variant| variant.is_used())
                            .flat_map(|variant| {
                                variant.fields.iter().filter(|field| field.is_default())
                            })
                            .collect_type_params(&options, &declared_type_params);
                        bounds.extend(type_params.into_iter().map(|ident| bound.apply(ident)));
                    }
                }
            }
        }

        if bounds.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounds),* }
        }
    }

    /// Get enum representation
    pub fn enum_repr(&self) -> EnumRepr<'_> {
        use EnumRepr::*;
        if self.untagged {
            if self.tag.is_some() {
                warning!(
                    self.ident.span(),
                    "Because `untagged` enum representation is used so the `tag` is ignored"
                );
            }
            if self.content.is_some() {
                warning!(
                    self.ident.span(),
                    "Because `untagged` enum representation is used so the `content` is ignored"
                );
            }
            let mut constant = false;
            if let Data::Enum(variants) = &self.data {
                let units = variants
                    .iter()
                    .filter(|variant| variant.fields.style == Style::Unit)
                    .count();
                if units == variants.len() {
                    constant = true;
                } else if units > 1 {
                    warning!(self.ident, "Multiple unit variants appears");
                }
            }
            Untagged { constant }
        } else if let Some(tag) = &self.tag {
            let tag = tag
                .as_ref()
                .explicit()
                .map(|name| name.into())
                .unwrap_or_else(|| "tag".into());
            if let Some(content) = &self.content {
                let content = content
                    .as_ref()
                    .explicit()
                    .map(|name| name.into())
                    .unwrap_or_else(|| "content".into());
                AdjacentlyTagged { tag, content }
            } else {
                InternallyTagged { tag }
            }
        } else {
            if self.content.is_some() {
                warning!(
                    self.ident.span(),
                    "Because `externally tagged` enum representation is used so the `content` is ignored. You would also set `tag` to get `adjacently tagged` enums"
                );
            }
            ExternallyTagged
        }
    }

    /// Get JS name for field or variant
    pub fn name_for<'a, T: Named>(&self, target: &'a T) -> Option<Cow<'a, str>> {
        target.name().or_else(|| target.ident(&self.rename_all))
    }
}

impl GenericsExt for DataType {
    fn declared_lifetimes(&self) -> LifetimeSet {
        self.generics
            .params
            .iter()
            .filter_map(|param| param.as_lifetime_def())
            .map(|lt| lt.lifetime.clone())
            .collect()
    }

    fn declared_type_params(&self) -> IdentSet {
        self.generics
            .type_params()
            .map(|tp| tp.ident.clone())
            .collect()
    }
}

#[derive(Debug, FromTypeParam)]
#[darling(attributes(quickjs))]
pub struct DataParam {
    /// The param ident
    ident: Ident,

    /// The param bounds
    bounds: Vec<TypeParamBound>,
}

impl ToTokens for DataParam {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens);
        if !self.bounds.is_empty() {
            let bounds = &self.bounds;
            quote!(
                : #(#bounds)+*
            )
            .to_tokens(tokens)
        }
    }
}

#[derive(Debug, FromVariant)]
#[darling(attributes(quickjs))]
pub struct DataVariant {
    /// The variant ident
    pub ident: Ident,

    /// The variant contents
    pub fields: Fields<DataField>,

    /// The discriminant
    pub discriminant: Option<Expr>,

    /// The variant name
    #[darling(default, rename = "rename")]
    pub name: Option<String>,

    /// Skip this variant
    #[darling(default)]
    pub skip: bool,
}

uses_lifetimes!(DataVariant, fields);
uses_type_params!(DataVariant, fields);

impl Named for DataVariant {
    fn ident(&self, rule: &RenameRule) -> Option<Cow<'_, str>> {
        Some(rule.apply_to_variant(self.ident.to_string()).into())
    }

    fn name(&self) -> Option<Cow<'_, str>> {
        self.name.as_ref().map(|name| name.into())
    }
}

impl DataVariant {
    pub fn is_used(&self) -> bool {
        !self.skip
    }
}

#[derive(Debug, FromField)]
#[darling(attributes(quickjs))]
pub struct DataField {
    /// The field visibility
    pub vis: Visibility,

    /// The field ident
    pub ident: Option<Ident>,

    /// The field type
    pub ty: Type,

    /// The field name
    #[darling(default, rename = "rename")]
    pub name: Option<String>,

    /// The default value of field
    #[darling(default)]
    pub default: Option<Override<Path>>,

    /// The field has references
    #[darling(default)]
    pub has_refs: bool,

    /// Skip this field
    #[darling(default)]
    pub skip: bool,
}

uses_lifetimes!(DataField, ty);
uses_type_params!(DataField, ty);

impl DataField {
    pub fn is_used(&self) -> bool {
        !self.skip
    }

    pub fn is_default(&self) -> bool {
        self.skip
            && self
                .default
                .as_ref()
                .map(|default| !default.is_explicit())
                .unwrap_or(true)
    }

    pub fn default(&self) -> TokenStream {
        match &self.default {
            Some(default) if default.is_explicit() => {
                let default = default.as_ref().explicit().unwrap();
                quote! { #default() }
            }
            _ => quote! { Default::default() },
        }
    }
}

impl Named for DataField {
    fn ident(&self, rule: &RenameRule) -> Option<Cow<'_, str>> {
        self.ident
            .as_ref()
            .map(|ident| rule.apply_to_field(ident.to_string()).into())
    }

    fn name(&self) -> Option<Cow<'_, str>> {
        self.name.as_ref().map(|name| name.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use darling::{
        ast::{GenericParamExt, Style},
        usage::{CollectLifetimes, CollectTypeParams, Purpose},
    };
    use syn::parse_quote;

    #[cfg(test)]
    macro_rules! tests {
        ($c:ident { $($s:tt)* } ($var:ident) { $($d:tt)* }; $($r:tt)*) => {
            #[test]
            fn $c() {
                let input = syn::parse_quote! { $($s)* };
                let output = DataType::from_derive_input(&input).unwrap();
                (|$var: DataType| { $($d)* })(output);
            }

            tests! { $($r)* }
        };

        ($c:ident { $($s:tt)* } $d:literal; $($r:tt)*) => {
            tests! { $c { $($s)* } (out) { assert_eq!(format!("{:?}", out), $d); }; $($r)* }
        };

        () => {};
    }

    tests! {
        unit_struct {
            struct Unit;
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Unit);
            assert_eq!(data.fields.len(), 0);
        };

        tuple_struct {
            struct Tuple(i32);
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Tuple);
            assert_eq!(data.fields.len(), 1);
            let field = &data.fields[0];
            assert_eq!(field.ty, parse_quote!(i32));
            assert!(field.name.is_none());
            assert!(!field.skip);
        };

        tuple_struct_empty {
            struct Tuple();
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Tuple);
            assert_eq!(data.fields.len(), 0);
        };

        tuple_struct_name_field {
            struct Tuple(#[quickjs(rename = "x")] f32);
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.fields.len(), 1);
            assert_eq!(data.fields[0].name.as_ref().unwrap(), "x");
        };

        tuple_struct_skip_field {
            struct Tuple(#[quickjs(skip)] i32);
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.fields.len(), 1);
            assert!(data.fields[0].skip);
        };

        struct_with_fields {
            struct Struct { x: f32 }
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Struct);
            assert_eq!(data.fields.len(), 1);
            let field = &data.fields[0];
            assert_eq!(field.ident.as_ref().unwrap(), "x");
            assert_eq!(field.ty, parse_quote!(f32));
            assert!(field.name.is_none());
            assert!(!field.skip);
        };

        struct_with_fields_empty {
            struct Struct { }
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Struct);
            assert_eq!(data.fields.len(), 0);
        };

        struct_with_fields_skipped {
            struct Struct { #[quickjs(skip)] x: i32 }
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Struct);
            assert_eq!(data.fields.len(), 1);
            assert!(data.fields[0].skip);
        };

        struct_with_fields_renamed {
            struct Struct { #[quickjs(rename = "X")] x: i32 }
        } (out) {
            let data = &out.data.take_struct().unwrap();
            assert_eq!(data.style, Style::Struct);
            assert_eq!(data.fields.len(), 1);
            assert_eq!(data.fields[0].name.as_ref().unwrap(), "X");
            assert_eq!(data.fields[0].name().unwrap(), "X");
        };

        generic_struct {
            struct Struct<T> { x: T }
        } (out) {
            let declared_type_params = out.declared_type_params();
            assert_eq!(declared_type_params.len(), 1);
            let declared_lifetimes = out.declared_lifetimes();
            assert_eq!(declared_lifetimes.len(), 0);

            let generics = &out.generics;
            let data = &out.data.take_struct().unwrap();

            let params = &generics.params;
            assert_eq!(params.len(), 1);
            let param = params[0].as_type_param().unwrap();
            assert_eq!(&param.ident, "T");
            assert_eq!(param.bounds.len(), 0);

            let fields = &data.fields;
            assert_eq!(fields.len(), 1);
            let field = &fields[0];
            assert_eq!(field.ident.as_ref().unwrap(), "x");
            assert_eq!(&field.ty, &parse_quote!(T));

            let options = Purpose::BoundImpl.into();
            let type_params = data.fields.collect_type_params(&options, &declared_type_params);
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params.iter().next().unwrap(), &"T");

            let lifetimes = data.fields.collect_lifetimes(&options, &declared_lifetimes);
            assert_eq!(lifetimes.len(), 0);
        };

        generic_struct_with_lifetime {
            struct Struct<'a, T> { x: &'a T }
        } (out) {
            let declared_type_params = out.declared_type_params();
            assert_eq!(declared_type_params.len(), 1);
            let declared_lifetimes = out.declared_lifetimes();
            assert_eq!(declared_lifetimes.len(), 1);

            let generics = &out.generics;
            let data = &out.data.take_struct().unwrap();

            let params = &generics.params;
            assert_eq!(params.len(), 2);
            let param = params[0].as_lifetime_def().unwrap();
            assert_eq!(&param.lifetime.ident, "a");
            assert_eq!(param.bounds.len(), 0);
            let param = params[1].as_type_param().unwrap();
            assert_eq!(&param.ident, "T");
            assert_eq!(param.bounds.len(), 0);

            let fields = &data.fields;
            assert_eq!(fields.len(), 1);
            let field = &fields[0];
            assert_eq!(field.ident.as_ref().unwrap(), "x");
            assert_eq!(&field.ty, &parse_quote!(&'a T));

            let options = Purpose::BoundImpl.into();
            let type_params = data.fields.collect_type_params(&options, &declared_type_params);
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params.iter().next().unwrap(), &"T");

            let lifetimes = data.fields.collect_lifetimes(&options, &declared_lifetimes);
            assert_eq!(lifetimes.len(), 1);
            assert_eq!(lifetimes.iter().next().unwrap().ident, &"a");
        };

        generic_struct_with_phantom_data {
            struct Struct<T> { x: PhantomData<T> }
        } (out) {
            let declared_type_params = out.declared_type_params();
            assert_eq!(declared_type_params.len(), 1);

            let generics = &out.generics;
            let data = &out.data.take_struct().unwrap();

            let params = &generics.params;
            assert_eq!(params.len(), 1);
            let param = params[0].as_type_param().unwrap();
            assert_eq!(&param.ident, "T");
            assert_eq!(param.bounds.len(), 0);

            let fields = &data.fields;
            assert_eq!(fields.len(), 1);
            let field = &fields[0];
            assert_eq!(field.ident.as_ref().unwrap(), "x");
            assert_eq!(&field.ty, &parse_quote!(PhantomData<T>));

            let options = Purpose::BoundImpl.into();
            let type_params = data.fields.collect_type_params(&options, &declared_type_params);
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params.iter().next().unwrap(), &"T");
        };

        rename_crate {
            #[quickjs(crate = "rquickjs2")]
            struct Data;
        } (out) {
            assert_eq!(out.crate_.as_ref().unwrap(), "rquickjs2");
        };

        impl_params_without_params {
            struct Data;
        } (out) {
            assert_eq!(out.impl_params(true).to_string(), quote!('js).to_string());
        };

        impl_params_with_lifetimes {
            struct Data<'a: 'b, 'b>;
        } (out) {
            assert_eq!(out.impl_params(true).to_string(), quote!('js, 'a: 'b, 'b).to_string());
        };

        impl_params_with_params {
            struct Data<A, B: From<f32> + From<i32>>;
        } (out) {
            assert_eq!(out.impl_params(true).to_string(), quote!('js, A, B: From<f32> + From<i32>).to_string());
        };

        impl_params_with_js_lifetime {
            struct Data<'js>;
        } (out) {
            assert_eq!(out.impl_params(true).to_string(), quote!('js).to_string());
        };

        type_name_without_params {
            struct Data;
        } (out) {
            assert_eq!(out.type_name().to_string(), quote!(Data).to_string());
        };

        type_name_with_lifetimes {
            struct Data<'a: 'b, 'b>;
        } (out) {
            assert_eq!(out.type_name().to_string(), quote!(Data<'a, 'b>).to_string());
        };

        type_name_with_params {
            struct Data<A, B: Into<A>>;
        } (out) {
            assert_eq!(out.type_name().to_string(), quote!(Data<A, B>).to_string());
        };

        where_clause {
            struct Data;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: IntoJs<'js>)), None).to_string(), quote!().to_string());
        };

        where_clause_unused {
            struct Data<T>;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: IntoJs<'js>)), None).to_string(), quote!().to_string());
        };

        where_clause_need_bound {
            struct Data<T>(T);
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: IntoJs<'js>)), None).to_string(), quote!(where T: IntoJs<'js>).to_string());
        };

        where_clause_need_bound_and_withou_skip {
            struct Data<T>(T, #[quickjs(skip)] T);
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: IntoJs<'js>)), None).to_string(), quote!(where T: IntoJs<'js>).to_string());
        };

        where_clause_need_bound_and_with_skip {
            struct Data<T>(T, #[quickjs(skip)] T);
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: FromJs<'js>)), Some(parse_quote!(T: Default))).to_string(), quote!(where T: FromJs<'js>, T: Default).to_string());
        };

        where_clause_with_type_where {
            struct Data<T>(T, #[quickjs(skip)] T) where T: AsType;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: FromJs<'js>)), Some(parse_quote!(T: Default))).to_string(), quote!(where T: AsType, T: FromJs<'js>, T: Default).to_string());
        };

        where_clause_overriden_bound {
            #[quickjs(bound = "T: FromStr + Default")]
            struct Data<T>(T, #[quickjs(skip)] T) where T: AsType;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: FromJs<'js>)), Some(parse_quote!(T: Default))).to_string(), quote!(where T: AsType, T: FromStr + Default).to_string());
        };

        where_clause_overriden_default_implicit {
            struct Data<T>(T, #[quickjs(skip, default)] T) where T: AsType;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: FromJs<'js>)), Some(parse_quote!(T: Default))).to_string(), quote!(where T: AsType, T: FromJs<'js>, T: Default).to_string());
        };

        where_clause_overriden_default_explicit {
            struct Data<T>(T, #[quickjs(skip, default = "T::new")] T) where T: AsType;
        } (out) {
            assert_eq!(out.where_clause(Some(parse_quote!(T: FromJs<'js>)), Some(parse_quote!(T: Default))).to_string(), quote!(where T: AsType, T: FromJs<'js>).to_string());
        };

        name_for_field_from_ident {
            struct Data {
                some_num: f32,
            }
        } (out) {
            let data = &out.data.as_ref().take_struct().unwrap();
            let field = data.fields[0];
            assert_eq!(out.name_for(field).unwrap(), "some_num");
        };

        name_for_field_renamed {
            struct Data {
                #[quickjs(rename = "another_num")]
                some_num: f32,
            }
        } (out) {
            let data = &out.data.as_ref().take_struct().unwrap();
            let field = data.fields[0];
            assert_eq!(out.name_for(field).unwrap(), "another_num");
        };

        name_for_field_rename_all {
            #[quickjs(rename_all = "camelCase")]
            struct Data {
                some_num: f32,
            }
        } (out) {
            let data = &out.data.as_ref().take_struct().unwrap();
            let field = data.fields[0];
            assert_eq!(out.name_for(field).unwrap(), "someNum");
        };

        name_for_field_rename_all_but_renamed {
            #[quickjs(rename_all = "camelCase")]
            struct Data {
                #[quickjs(rename = "anotherNum")]
                some_num: f32,
            }
        } (out) {
            let data = &out.data.as_ref().take_struct().unwrap();
            let field = data.fields[0];
            assert_eq!(out.name_for(field).unwrap(), "anotherNum");
        };

        name_for_variant_from_ident {
            enum Data {
                SomeVariant,
            }
        } (out) {
            let data = &out.data.as_ref().take_enum().unwrap();
            let variant = data[0];
            assert_eq!(out.name_for(variant).unwrap(), "SomeVariant");
        };
    }
}
