use crate::{loader::Loader, Ctx, Error, ModuleData, Result};
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
    #[must_use]
    pub fn with_module<N: Into<String>, S: Into<Vec<u8>>>(mut self, name: N, source: S) -> Self {
        self.add_module(name, source);
        self
    }
}

impl Loader for BuiltinLoader {
    fn load<'js>(&mut self, _ctx: Ctx<'js>, path: &str) -> Result<ModuleData> {
        match self.modules.remove(path) {
            Some(source) => Ok(ModuleData::source(path, source)),
            _ => Err(Error::new_loading(path)),
        }
    }
}
