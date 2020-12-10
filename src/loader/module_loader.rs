use crate::{BeforeInit, Ctx, Error, Loader, Module, ModuleDef, Result};
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter, Result as FmtResult},
};

type ModuleInitFn = dyn for<'js> FnOnce(Ctx<'js>, &str) -> Result<Module<'js, BeforeInit>>;

struct ModuleInit(Box<ModuleInitFn>);

impl Debug for ModuleInit {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        "<native>".fmt(f)
    }
}

/// The builtin native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[derive(Debug)]
pub struct ModuleLoader {
    modules: HashMap<String, ModuleInit>,
}

impl ModuleLoader {
    /// Add module
    pub fn add_module<N: Into<String>, M: ModuleDef>(&mut self, name: N, _module: M) -> &mut Self {
        self.modules.insert(
            name.into(),
            #[allow(clippy::redundant_closure)]
            ModuleInit(Box::new(|ctx, name| Module::new::<M, _>(ctx, name))),
        );
        self
    }

    /// Add module
    pub fn with_module<N: Into<String>, M: ModuleDef>(mut self, name: N, module: M) -> Self {
        self.add_module(name, module);
        self
    }
}

impl Default for ModuleLoader {
    fn default() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }
}

impl Loader for ModuleLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, BeforeInit>> {
        match self.modules.remove(path) {
            Some(module_init) => module_init.0(ctx, path),
            _ => Err(Error::new_loading(path)),
        }
    }
}
