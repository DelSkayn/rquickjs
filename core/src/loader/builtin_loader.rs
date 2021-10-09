use crate::{Ctx, Error, Loaded, Loader, Module, Result, Script};
use std::collections::HashMap;

/// The builtin script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Debug, Default)]
pub struct BuiltinLoader {
    modules: HashMap<String, Vec<u8>>,
}

impl BuiltinLoader {
    /// Add builtin script module
    pub fn add_module<N: Into<String>, S: Into<Vec<u8>>>(
        &mut self,
        name: N,
        source: S,
    ) -> &mut Self {
        self.modules.insert(name.into(), source.into());
        self
    }

    /// Add builtin script module
    pub fn with_module<N: Into<String>, S: Into<Vec<u8>>>(mut self, name: N, source: S) -> Self {
        self.add_module(name, source);
        self
    }
}

impl Loader<Script> for BuiltinLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, Loaded<Script>>> {
        match self.modules.remove(path) {
            Some(source) => Module::new(ctx, path, source),
            _ => Err(Error::new_loading(path)),
        }
    }
}
