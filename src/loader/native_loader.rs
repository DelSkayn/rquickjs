use super::check_extensions;
use crate::{qjs, BeforeInit, Ctx, Error, Loader, Module, Result};

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
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, BeforeInit>> {
        use dlopen::raw::Library;
        use std::ffi::CString;

        if !check_extensions(&path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        type LoadFn =
            unsafe extern "C" fn(*mut qjs::JSContext, *const qjs::c_char) -> *mut qjs::JSModuleDef;

        let lib = Library::open(&path)
            .map_err(|_| Error::new_loading_message(path, "Unable to open library"))?;
        let load_fn: LoadFn = unsafe { lib.symbol("js_init_module") }.map_err(|_| {
            Error::new_loading_message(path, "Unable to find symbol `js_init_module`")
        })?;

        let name = CString::new(path)?;
        let ptr = unsafe { load_fn(ctx.ctx, name.as_ptr()) };

        if ptr.is_null() {
            Err(Error::Unknown)
        } else {
            self.libs.push(lib);
            Ok(unsafe { Module::from_module_def(ctx, ptr) })
        }
    }
}
