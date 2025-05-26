use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, Ident, ItemFn, Result, ReturnType, spanned::Spanned};

/// make sure the declare function has the right type.
pub fn validate(func: &ItemFn) -> Result<()> {
    let sig = &func.sig;
    if let Some(x) = sig.asyncness.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module evaluation function can't be async.",
        ));
    }
    if let Some(x) = sig.unsafety.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module evaluation function can't be unsafe.",
        ));
    }
    if let Some(x) = sig.abi.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module evaluation function can't have an abi.",
        ));
    }
    if sig.inputs.len() != 2 || sig.output == ReturnType::Default {
        return Err(Error::new(
            func.span(),
            "Invalid module evaluation function. Function should implement `fn(rquickjs::Ctx,&mut rquickjs::module::Exports) -> rquickjs::result<()>`.",
        ));
    }

    Ok(())
}

pub(crate) fn expand_use(module_name: &Ident, func: &ItemFn) -> TokenStream {
    let func_name = &func.sig.ident;
    quote! {
        #module_name::#func_name(_ctx,_exports)?;
    }
}
