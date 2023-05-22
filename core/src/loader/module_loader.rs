use crate::{Ctx, Error, ModuleData, ModuleDef, Result};
use std::{collections::HashMap, fmt::Debug};

use super::Loader;

/// The builtin native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Debug, Default)]
pub struct ModuleLoader {
    modules: HashMap<String, ModuleData>,
}

impl ModuleLoader {
    /// Add module
    pub fn add_module<N: Into<String>, M: ModuleDef>(&mut self, name: N, _module: M) -> &mut Self {
        let name = name.into();
        let data = ModuleData::native::<M, _>(name.clone());

        self.modules.insert(name, data);
        self
    }

    /// Add module
    #[must_use]
    pub fn with_module<N: Into<String>, M: ModuleDef>(mut self, name: N, module: M) -> Self {
        self.add_module(name, module);
        self
    }
}

impl Loader for ModuleLoader {
    fn load<'js>(&mut self, _ctx: Ctx<'js>, path: &str) -> Result<ModuleData> {
        self.modules
            .remove(path)
            .ok_or_else(|| Error::new_loading(path))
    }
}
