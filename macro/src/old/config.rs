use quote::format_ident;
use syn::Ident;

#[derive(Debug, Clone)]
pub struct Config {
    pub lib_crate: Ident,
    pub bind_attr: Ident,
    pub declare_var: Ident,
    pub exports_var: Ident,
}

pub fn lib_crate() -> String {
    env!("CARGO_PKG_NAME").replace("-macro", "")
}

impl Default for Config {
    fn default() -> Self {
        let lib_crate = lib_crate();
        Self {
            lib_crate: format_ident!("{}", lib_crate),
            bind_attr: format_ident!("quickjs"),
            declare_var: format_ident!("declares"),
            exports_var: format_ident!("exports"),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        use proc_macro_crate::{crate_name, FoundCrate};

        let lib_crate = crate_name(&lib_crate())
            .unwrap_or_else(|error| abort!("Unable to determine lib crate name: {}", error));
        let lib_crate = match lib_crate {
            FoundCrate::Name(name) => name,
            _ => unreachable!(),
        };
        Self {
            lib_crate: format_ident!("{}", lib_crate),
            ..Default::default()
        }
    }
}
