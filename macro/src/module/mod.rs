use std::collections::{hash_map::Entry, HashMap};

use crate::{
    attrs::{take_attributes, OptionList},
    class::Class,
};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, Attribute, Error, ItemFn, ItemMod, Result, UseTree};

mod config;
mod declare;
mod evaluate;
pub(crate) use config::*;

#[derive(Debug)]
pub(crate) struct JsModule {
    pub config: ModuleConfig,
    pub name: Ident,
    pub declaration: HashMap<String, (Span, TokenStream)>,
}

impl JsModule {
    pub fn new(config: ModuleConfig, item: &ItemMod) -> Self {
        JsModule {
            config,
            name: item.ident.clone(),
            declaration: HashMap::new(),
        }
    }

    pub fn export(&mut self, name: String, span: Span, tokens: TokenStream) {
        match self.declaration.entry(name) {
            Entry::Occupied(mut x) => {
                /*
                let first_span = x.get().0;
                emit_warning!(
                    span,"Module export with name `{}` already exists.", x.key();
                    note = first_span => "First declared here.";
                    info = "Exporting two values with the same name will cause the second value to overwrite the first"
                );
                */
                *x.get_mut() = (span, tokens);
            }
            Entry::Vacant(x) => {
                x.insert((span, tokens));
            }
        }
    }

    pub fn expand_declarations(&mut self) -> TokenStream {
        let keys = self.declaration.keys();

        quote! {
            #(_declare.declare(#keys)?;)*
        }
    }

    pub fn expand_exports(&mut self) -> TokenStream {
        let values = self.declaration.values().map(|x| &x.1);
        quote! {
            #(#values)*
        }
    }
}

fn parse_item_attrs(attrs: &mut Vec<Attribute>) -> Result<ModuleItemConfig> {
    let mut config = ModuleItemConfig::default();

    take_attributes(attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<ModuleItemOption> = attr.parse_args()?;
        for option in options.0.iter() {
            config.apply(option)
        }

        Ok(true)
    })?;

    Ok(config)
}

fn parse_type_attrs(attrs: &mut Vec<Attribute>) -> Result<ModuleTypeConfig> {
    let mut config = ModuleTypeConfig::default();

    take_attributes(attrs, |attr| {
        if attr.path().is_ident("qjs") {
            let options: OptionList<ModuleTypeOption> = attr.parse_args()?;
            for option in options.0.iter() {
                config.apply(option)
            }

            return Ok(true);
        }

        if attr
            .path()
            .segments
            .last()
            .map(|x| x.ident == "class")
            .unwrap_or(false)
        {
            config.class_macro_name = Some(attr.path().clone());
            let options: OptionList<ModuleTypeOption> = attr.parse_args()?;
            for option in options.0.iter() {
                config.apply(option)
            }
        }

        Ok(false)
    })?;

    Ok(config)
}

fn export_use(use_: &UseTree, module: &mut JsModule, config: &ModuleItemConfig) -> Result<()> {
    match use_ {
        UseTree::Path(x) => {
            export_use(&x.tree, module, config)?;
        }
        UseTree::Name(x) => {
            let ident = &x.ident;
            let js_name = config.js_name(ident, module.config.rename_types);
            let crate_name = Ident::new(&module.config.crate_name()?, ident.span());
            let mod_name = module.name.clone();
            module.export(
                js_name.clone(),
                ident.span(),
                quote! {
                    let _constr = #crate_name::Class::<#mod_name::#ident>::create_constructor(&_ctx)?
                        .expect(concat!("Tried to export type `"
                                ,stringify!(#ident),
                                "` which did not define a constructor."
                        ));
                    _exports.export(#js_name,_constr)?;
                },
                )
        }
        UseTree::Rename(x) => {
            let ident = &x.rename;
            let js_name = config.js_name(ident, module.config.rename_types);
            let crate_name = Ident::new(&module.config.crate_name()?, ident.span());
            let mod_name = module.name.clone();
            module.export(
                js_name.clone(),
                ident.span(),
                quote! {
                    let _constr = #crate_name::Class::<#mod_name::#ident>::create_constructor(&_ctx)?
                        .expect("Tried to export type which did not define a constructor.");
                    _exports.export(#js_name,_constr)?;
                },
                )
        }
        UseTree::Glob(x) => {
            return Err(Error::new(x.star_token.span(),"Using a glob export does not export the items to JavaScript.Please specify each item to be exported individially."))
        }
        UseTree::Group(x) => {
            for i in x.items.iter() {
                export_use(i, module, config)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn expand(options: OptionList<ModuleOption>, mut item: ItemMod) -> Result<TokenStream> {
    let mut config = ModuleConfig::default();
    for option in options.0.iter() {
        config.apply(option)
    }

    let ItemMod { ref mut attrs, .. } = item;

    take_attributes(attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<ModuleOption> = attr.parse_args()?;
        for option in options.0.iter() {
            config.apply(option)
        }

        Ok(true)
    })?;

    let mut module = JsModule::new(config, &item);

    let ItemMod {
        ref mut content,
        ref unsafety,
        ..
    } = item;

    if let Some(unsafe_) = unsafety {
        return Err(Error::new(
            unsafe_.span(),
            "unsafe modules are not supported",
        ));
    }

    let Some((_, ref mut items)) = content else {
        return Err(Error::new(
            Span::call_site(),
            "The `module` macro can only be applied to modules with a definition in the same file.",
        ));
    };

    let mut _consts = Vec::new();
    let mut _statics = Vec::new();
    let mut _enums = Vec::new();
    let mut _structs = Vec::new();
    let mut _uses = Vec::new();
    let mut _functions = Vec::new();

    let mut declare: Option<(&ItemFn, Span, ModuleFunctionConfig)> = None;
    let mut evaluate: Option<(&ItemFn, Span, ModuleFunctionConfig)> = None;

    for item in items.iter_mut() {
        match item {
            syn::Item::Use(i) => {
                let config = parse_item_attrs(&mut i.attrs)?;
                if config.skip {
                    continue;
                }

                if let syn::Visibility::Public(_) = i.vis {
                    _uses.push((i, config))
                }
            }
            syn::Item::Const(i) => {
                let config = parse_item_attrs(&mut i.attrs)?;
                if config.skip {
                    continue;
                }
                if let syn::Visibility::Public(_) = i.vis {
                    _consts.push((i, config))
                }
            }
            syn::Item::Static(i) => {
                let config = parse_item_attrs(&mut i.attrs)?;
                if config.skip {
                    continue;
                }
                if let syn::Visibility::Public(_) = i.vis {
                    _statics.push((i, config))
                }
            }
            syn::Item::Enum(i) => {
                let config = parse_type_attrs(&mut i.attrs)?;
                if config.skip {
                    continue;
                }

                if let Some(reexport) = config.reexpand() {
                    i.attrs.push(reexport);
                }

                if let syn::Visibility::Public(_) = i.vis {
                    _enums.push((i, config))
                }
            }
            syn::Item::Fn(ref mut i) => {
                let mut config = ModuleFunctionConfig::default();
                let mut span = None;

                take_attributes(&mut i.attrs, |attr| {
                    if !attr.path().is_ident("qjs") {
                        return Ok(false);
                    }

                    span = Some(attr.span());

                    let options: OptionList<ModuleFunctionOption> = attr.parse_args()?;
                    for option in options.0.iter() {
                        config.apply(option)
                    }

                    Ok(true)
                })?;

                config.validate(span.unwrap_or_else(Span::call_site))?;

                if config.skip {
                    continue;
                } else if config.declare {
                    let span = span.unwrap();
                    if let Some((_, prev_span, _)) = declare {
                        let mut error =
                            Error::new(span, "Found a second declaration function in module.");
                        error.combine(Error::new(prev_span, "First declaration function here."));
                        return Err(error);
                    }
                    declare::validate(i)?;
                    declare = Some((i, span, config));
                } else if config.evaluate {
                    let span = span.unwrap();
                    if let Some((_, prev_span, _)) = evaluate {
                        let mut error =
                            Error::new(span, "Found a second declaration function in module.");
                        error.combine(Error::new(prev_span, "First declaration function here."));
                        return Err(error);
                    }
                    evaluate::validate(i)?;
                    evaluate = Some((i, span, config));
                } else {
                    if let Some(reexport) = config.reexpand() {
                        i.attrs.push(reexport);
                    }

                    if let syn::Visibility::Public(_) = i.vis {
                        _functions.push((i, config))
                    }
                }
            }
            syn::Item::Struct(i) => {
                let config = parse_type_attrs(&mut i.attrs)?;
                if config.skip {
                    continue;
                }

                if let Some(reexport) = config.reexpand() {
                    i.attrs.push(reexport);
                }

                if let syn::Visibility::Public(_) = i.vis {
                    _structs.push((i, config))
                }
            }
            syn::Item::Trait(_)
            | syn::Item::TraitAlias(_)
            | syn::Item::Type(_)
            | syn::Item::Union(_)
            | syn::Item::Verbatim(_)
            | syn::Item::ExternCrate(_)
            | syn::Item::Impl(_)
            | syn::Item::Macro(_)
            | syn::Item::ForeignMod(_)
            | syn::Item::Mod(_) => {}
            _ => {}
        }
    }

    let mod_name = &item.ident;
    let crate_name = Ident::new(&module.config.crate_name()?, Span::call_site());
    let name = module.config.carry_name(&item.ident);
    let vis = item.vis.clone();

    let declare = declare.map(|x| declare::expand_use(mod_name, x.0));
    let evaluate = evaluate.map(|x| evaluate::expand_use(mod_name, x.0));

    for (f, function_config) in _functions {
        let ident = function_config.function.carry_name(&f.sig.ident);
        let js_name = function_config
            .function
            .js_name(&f.sig.ident, module.config.rename_types);

        let mod_name = module.name.clone();

        module.export(
            js_name.clone(),
            f.sig.ident.span(),
            quote! {
                _exports.export(#js_name,#mod_name::#ident)?;
            },
        )
    }

    for (c, config) in _consts {
        let ident = &c.ident;
        let js_name = config.js_name(ident, module.config.rename_vars);
        module.export(
            js_name.clone(),
            ident.span(),
            quote! {
                _exports.export(#js_name,#mod_name::#ident)?;
            },
        )
    }

    for (s, config) in _statics {
        let ident = &s.ident;
        let js_name = config.js_name(ident, module.config.rename_vars);
        module.export(
            js_name.clone(),
            ident.span(),
            quote! {
                _exports.export(#js_name,#mod_name::#ident)?;
            },
        )
    }

    for (s, config) in _structs {
        let ident = &s.ident;
        let name = Class::from_struct(config.class.clone(), s.clone())?.javascript_name();

        module.export(
            name.clone(),
            ident.span(),
            quote! {
                let _constr = #crate_name::Class::<#mod_name::#ident>::create_constructor(&_ctx)?
                    .expect(concat!("Tried to export type `"
                            ,stringify!(#ident),
                            "` which did not define a constructor."
                    ));
                _exports.export(#name,_constr)?;
            },
        )
    }
    for (e, config) in _enums {
        let ident = &e.ident;

        let name = Class::from_enum(config.class.clone(), e.clone())?.javascript_name();

        module.export(
            name.clone(),
            ident.span(),
            quote! {
                let _constr = #crate_name::Class::<#mod_name::#ident>::create_constructor(&_ctx)?
                    .expect(concat!("Tried to export type `"
                            ,stringify!(#ident),
                            "` which did not define a constructor."
                    ));
                _exports.export(#name,_constr)?;
            },
        )
    }

    for (u, config) in _uses {
        export_use(&u.tree, &mut module, &config)?;
    }

    let declarations = module.expand_declarations();
    let exports = module.expand_exports();
    let res = quote! {
        #[allow(non_camel_case_types)]
        #vis struct #name;

        impl #crate_name::module::ModuleDef for #name{
            fn declare(_declare: &#crate_name::module::Declarations) -> #crate_name::Result<()>{
                #declarations
                #declare
                Ok(())
            }
            fn evaluate<'js>(_ctx: &#crate_name::Ctx<'js>, _exports: &#crate_name::module::Exports<'js>) -> #crate_name::Result<()>{
                #exports
                #evaluate
                Ok(())
            }
        }

        #item
    };
    Ok(res)
}
