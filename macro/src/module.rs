use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, abort_call_site};
use syn::ItemMod;

use crate::common::Case;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    freeze: bool,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    rename: Option<String>,
    rename_constants: Option<Case>,
    rename_functions: Option<Case>,
}

pub(crate) fn expand(attr: AttrItem, item: ItemMod) -> TokenStream {
    let ItemMod { content, .. } = item;

    let Some(_content) = content else {
        abort_call_site!(
            "The `module` macro can only be applied to modules with a definition in the same file."
        )
    };

    todo!()
}
