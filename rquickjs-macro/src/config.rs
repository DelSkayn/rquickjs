use crate::abort;
use quote::format_ident;
use syn::Ident;

#[derive(Debug, Clone)]
pub struct Config {
    pub lib_crate: Ident,
    pub exports_var: Ident,
}

fn lib_crate() -> String {
    env!("CARGO_PKG_NAME").replace("-macro", "").into()
}

impl Default for Config {
    fn default() -> Self {
        let lib_crate = lib_crate();
        Self {
            lib_crate: format_ident!("{}", lib_crate),
            exports_var: format_ident!("exports"),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        let lib_crate = proc_macro_crate::crate_name(&lib_crate())
            .unwrap_or_else(|error| abort!("Unable to determine lib crate name: {}", error));
        Self {
            lib_crate: format_ident!("{}", lib_crate),
            ..Default::default()
        }
    }
}
