use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::quote;
use syn::{Ident, ItemFn, ReturnType};

/// make sure the declare function has the right type.
pub fn validate(func: &ItemFn) {
    let sig = &func.sig;
    if let Some(x) = sig.asyncness.as_ref() {
        abort!(x, "A module evaluation function can't be async.");
    }
    if let Some(x) = sig.unsafety.as_ref() {
        abort!(x, "A module evaluation function can't be unsafe.");
    }
    if let Some(x) = sig.abi.as_ref() {
        abort!(x, "A module evaluation function can't have an abi.");
    }
    if sig.inputs.len() != 2 || sig.output == ReturnType::Default {
        abort!(func, "Invalid module evaluation function.";
            note = "Function should implement `fn(rquickjs::Ctx,&mut rquickjs::module::Exports) -> rquickjs::result<()>`.");
    }
}

pub(crate) fn expand_use(module_name: &Ident, func: &ItemFn) -> TokenStream {
    let func_name = &func.sig.ident;
    quote! {
        #module_name::#func_name(_ctx,_exports)?;
    }
}
