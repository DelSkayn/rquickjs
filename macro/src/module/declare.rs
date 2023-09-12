use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::quote;
use syn::{Ident, ItemFn, ReturnType, Visibility};

/// make sure the declare function has the right type.
pub fn validate(func: &ItemFn) {
    let sig = &func.sig;
    if !matches!(func.vis, Visibility::Public(_)) {
        abort!(func.sig.ident, "A module declare function must be public.");
    }
    if let Some(x) = sig.asyncness.as_ref() {
        abort!(x, "A module declare function can't be async.");
    }
    if let Some(x) = sig.unsafety.as_ref() {
        abort!(x, "A module declare function can't be unsafe.");
    }
    if let Some(x) = sig.abi.as_ref() {
        abort!(x, "A module declare function can't have an abi.");
    }
    if sig.inputs.len() != 1 || sig.output == ReturnType::Default {
        abort!(func, "Invalid module declaration function.";
            note = "Function should implement `fn(&mut rquickjs::module::Declarations) -> rquickjs::result<()>`.");
    }
}

pub fn expand_use(module_name: &Ident, func: &ItemFn) -> TokenStream {
    let func_name = &func.sig.ident;
    quote! {
        #module_name::#func_name(_declare)?;
    }
}
