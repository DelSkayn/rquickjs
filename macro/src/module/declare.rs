use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned as _, Error, Ident, ItemFn, Result, ReturnType, Visibility};

/// make sure the declare function has the right type.
pub fn validate(func: &ItemFn) -> Result<()> {
    let sig = &func.sig;
    if !matches!(func.vis, Visibility::Public(_)) {
        Error::new(
            func.sig.ident.span(),
            "A module declare function must be public.",
        );
    }
    if let Some(x) = sig.asyncness.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module declare function can't be async.",
        ));
    }
    if let Some(x) = sig.unsafety.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module declare function can't be unsafe.",
        ));
    }
    if let Some(x) = sig.abi.as_ref() {
        return Err(Error::new(
            x.span(),
            "A module declare function can't have an abi.",
        ));
    }
    if sig.inputs.len() != 1 || sig.output == ReturnType::Default {
        return Err(Error::new(func.span(), "Invalid module declaration function. Function should implement `fn(&mut rquickjs::module::Declarations) -> rquickjs::result<()>`."));
    }

    Ok(())
}

pub fn expand_use(module_name: &Ident, func: &ItemFn) -> TokenStream {
    let func_name = &func.sig.ident;
    quote! {
        #module_name::#func_name(_declare)?;
    }
}
