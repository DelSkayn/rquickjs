use crate::{module::ModuleDef, Ctx, Error, Module, Result};
use alloc::{string::String, vec::Vec};
use core::fmt::Debug;
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use super::Loader;

type LoadFn = for<'js> fn(Ctx<'js>, Vec<u8>) -> Result<Module<'js>>;

/// The builtin native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[derive(Debug, Default)]
pub struct ModuleLoader {
    modules: HashMap<String, LoadFn>,
}

impl ModuleLoader {
    fn load_func<'js, D: ModuleDef>(ctx: Ctx<'js>, name: Vec<u8>) -> Result<Module<'js>> {
        Module::declare_def::<D, _>(ctx, name)
    }

    /// Add module
    pub fn add_module<N: Into<String>, M: ModuleDef>(&mut self, name: N, _module: M) -> &mut Self {
        self.modules.insert(name.into(), Self::load_func::<M>);
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
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        path: &str,
        _attributes: Option<crate::loader::ImportAttributes<'js>>,
    ) -> Result<Module<'js>> {
        let load = self
            .modules
            .remove(path)
            .ok_or_else(|| Error::new_loading(path))?;

        (load)(ctx.clone(), Vec::from(path))
    }
}
