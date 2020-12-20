use super::check_extensions;
use crate::{Ctx, Error, Loaded, Loader, Module, Result};
use std::collections::HashMap;

/// The compile module loader
///
/// This loader purposed to pre-compile modules which goes to be built in.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Debug)]
pub struct CompileLoader {
    extensions: Vec<String>,
    // collected modules bytecode
    modules: HashMap<String, Vec<u8>>,
}

impl CompileLoader {
    /// Add script file extension
    pub fn add_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.into());
        self
    }

    /// Add script file extension
    pub fn with_extension<X: Into<String>>(mut self, extension: X) -> Self {
        self.add_extension(extension);
        self
    }

    /// Get collected modules bytecode
    pub fn modules(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.modules
            .iter()
            .map(|(name, data)| (name.as_ref(), data.as_ref()))
    }
}

impl Default for CompileLoader {
    fn default() -> Self {
        Self {
            extensions: vec!["js".into()],
            modules: HashMap::new(),
        }
    }
}

impl Loader for CompileLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, Loaded>> {
        if !check_extensions(&path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        let source: Vec<_> = std::fs::read(&path)?;
        let module = Module::new(ctx, path, source)?;
        self.modules
            .insert(path.into(), module.write_object(false)?);
        Ok(module.into_loaded())
    }
}
