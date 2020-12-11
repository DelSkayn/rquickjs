use super::check_extensions;
use crate::{BeforeInit, Ctx, Error, Loader, Module, Result};

/// The script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Debug)]
pub struct ScriptLoader {
    extensions: Vec<String>,
}

impl ScriptLoader {
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
}

impl Default for ScriptLoader {
    fn default() -> Self {
        Self {
            extensions: vec!["js".into()],
        }
    }
}

impl Loader for ScriptLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, BeforeInit>> {
        if !check_extensions(&path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        let source: Vec<_> = std::fs::read(&path)?;
        ctx.compile_only(path, source)
    }
}
