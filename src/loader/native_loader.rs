use super::check_extensions;
use crate::{Ctx, Error, Loaded, Loader, Module, ModuleLoadFn, Result};

/// The native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "dyn-load")))]
#[derive(Debug)]
pub struct NativeLoader {
    extensions: Vec<String>,
    libs: Vec<dlopen::raw::Library>,
}

impl NativeLoader {
    /// Add library file extension
    pub fn add_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.into());
        self
    }

    /// Add library file extension
    pub fn with_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.add_extension(extension);
        self
    }
}

impl Default for NativeLoader {
    fn default() -> Self {
        let mut loader = Self {
            extensions: Vec::new(),
            libs: Vec::new(),
        };

        #[cfg(target_family = "windows")]
        loader.add_extension("dll");

        #[cfg(all(target_family = "unix"))]
        loader.add_extension("so");

        #[cfg(target_vendor = "apple")]
        loader.add_extension("dylib");

        loader
    }
}

impl Loader for NativeLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, Loaded>> {
        use dlopen::raw::Library;

        if !check_extensions(&path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        let lib = Library::open(&path)
            .map_err(|_| Error::new_loading_message(path, "Unable to open library"))?;
        let load: ModuleLoadFn = unsafe { lib.symbol("js_init_module") }.map_err(|_| {
            Error::new_loading_message(path, "Unable to find symbol `js_init_module`")
        })?;

        let module = unsafe { Module::new_raw(ctx, path, load) }
            .map_err(|_| Error::new_loading_message(path, "Unable to create module"))?;

        self.libs.push(lib);

        Ok(module.into_loaded())
    }
}
