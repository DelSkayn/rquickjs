use alloc::{string::String, vec, vec::Vec};

#[cfg(feature = "std")]
use crate::{
    loader::{util::check_extensions, Loader},
    Ctx, Error, Module, Result,
};

/// The script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
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
    #[must_use]
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

#[cfg(feature = "std")]
impl Loader for ScriptLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        path: &str,
        _attributes: crate::loader::ImportAttributes<'js>,
    ) -> Result<Module<'js>> {
        if !check_extensions(path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        let source: Vec<_> = std::fs::read(path)?;
        Module::declare(ctx.clone(), path, source)
    }
}
