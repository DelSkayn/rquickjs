use super::{BindConst, BindFn};
use crate::{abort, Config, TokenStream};
use quote::quote;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindProp {
    pub val: Option<BindConst>,
    pub get: Option<BindFn>,
    pub set: Option<BindFn>,
}

impl BindProp {
    pub fn expand(&self, cfg: &Config) -> TokenStream {
        match (&self.get, &self.set, &self.val) {
            (Some(get), Some(set), _) => {
                let get = get.expand(cfg);
                let set = set.expand(cfg);
                quote! { (#get, #set) }
            }
            (Some(get), _, _) => {
                let get = get.expand(cfg);
                quote! { (#get, ) }
            }
            (_, _, Some(val)) => {
                let val = val.expand(cfg);
                quote! { (#val, ) }
            }
            _ => {
                abort!("{}", "Misconfigured property");
            }
        }
    }
}
