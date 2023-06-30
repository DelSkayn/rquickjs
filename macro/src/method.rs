use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::quote;
use syn::{ImplItemFn, ItemImpl};

use crate::{crate_ident, function::JsFunction, Common};

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    prefix: Option<String>,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

pub(crate) fn expand(attr: AttrItem, item: ItemImpl) -> TokenStream {
    let ItemImpl {
        ref trait_,
        ref self_ty,
        ref items,
        ..
    } = item;

    if let Some(trait_) = trait_.as_ref() {
        abort!(
            trait_.2,
            "#[method] can't be applied to a trait implementation"
        );
    }

    let common = Common {
        prefix: attr.prefix.unwrap_or_else(|| "js_".to_string()),
        lib_crate: attr.crate_.unwrap_or_else(crate_ident),
    };

    let mut functions = Vec::new();
    //let mut consts = Vec::new();

    for item in items.iter() {
        match item {
            syn::ImplItem::Const(ref _item) => {}
            syn::ImplItem::Fn(ref item) => {
                let ImplItemFn { vis, sig, .. } = item;

                functions.push(JsFunction::new(vis.clone(), sig, Some(&**self_ty)))
            }
            _ => {}
        }
    }

    let func_common = Common {
        prefix: "__impl_".to_string(),
        lib_crate: common.lib_crate.clone(),
    };

    let function_impls = functions.iter().map(|func| {
        let carry_type = func.expand_carry_type(&func_common);
        let impl_ = func.expand_to_js_function_impl(&func_common);
        let into_js = func.expand_into_js_impl(&func_common);

        quote! {
            #carry_type

            #impl_

            #into_js
        }
    });

    let lib_crate = &common.lib_crate;

    let associated_types = functions.iter().map(|func| {
        let associated_name = func.expand_carry_type_name(&common);
        let impl_name = func.expand_carry_type_name(&func_common);
        let vis = &func.vis;

        quote! {
            #vis const #associated_name: #impl_name = #impl_name;
        }
    });

    let function_apply_proto = functions.iter().map(|func| {
        let func_name_str = &func.name;
        let js_func_name = func.expand_carry_type_name(&common);
        quote! {
            _proto.set(stringify!(#func_name_str),#self_ty::#js_func_name)?;
        }
    });

    quote! {
        #item

        #(#function_impls)*

        #[allow(non_upper_case_globals)]
        impl #self_ty{
            #(#associated_types)*
        }

        impl #lib_crate::class::impl_::MethodImplementor<#self_ty> for #lib_crate::class::impl_::MethodImpl<#self_ty> {
            fn implement<'js>(&self, _proto: &#lib_crate::Object<'js>) -> #lib_crate::Result<()>{
                #(#function_apply_proto)*
                Ok(())
            }
        }
    }
}
