use std::{ffi::CStr, ptr};

use crate::{qjs, Ctx, ModuleData, Result};

mod builtin_resolver;
pub use builtin_resolver::BuiltinResolver;

mod file_resolver;
pub use file_resolver::FileResolver;

mod script_loader;
pub use script_loader::ScriptLoader;

mod builtin_loader;
pub use builtin_loader::BuiltinLoader;

mod module_loader;
pub use module_loader::ModuleLoader;

#[cfg(feature = "dyn-load")]
mod native_loader;
#[cfg(feature = "dyn-load")]
pub use native_loader::NativeLoader;

pub mod util;

/// Module resolver interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub trait Resolver {
    /// Normalize module name
    ///
    /// The resolving may looks like:
    ///
    /// ```no_run
    /// # use rquickjs::{Ctx, Result, Error};
    /// # fn default_resolve<'js>(_ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
    /// Ok(if !name.starts_with('.') {
    ///     name.into()
    /// } else {
    ///     let mut split = base.rsplitn(2, '/');
    ///     let path = match (split.next(), split.next()) {
    ///         (_, Some(path)) => path,
    ///         _ => "",
    ///     };
    ///     format!("{path}/{name}")
    /// })
    /// # }
    /// ```
    fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String>;
}

/// Module loader interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub trait Loader {
    /// Load module by name
    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<ModuleData>;
}

struct LoaderOpaque {
    resolver: Box<dyn Resolver>,
    loader: Box<dyn Loader>,
}

#[repr(transparent)]
pub(crate) struct LoaderHolder(*mut LoaderOpaque);

impl Drop for LoaderHolder {
    fn drop(&mut self) {
        let _opaque = unsafe { Box::from_raw(self.0) };
    }
}

impl LoaderHolder {
    pub fn new<R, L>(resolver: R, loader: L) -> Self
    where
        R: Resolver + 'static,
        L: Loader + 'static,
    {
        Self(Box::into_raw(Box::new(LoaderOpaque {
            resolver: Box::new(resolver),
            loader: Box::new(loader),
        })))
    }

    pub(crate) fn set_to_runtime(&self, rt: *mut qjs::JSRuntime) {
        unsafe {
            qjs::JS_SetModuleLoaderFunc(
                rt,
                Some(Self::normalize_raw),
                Some(Self::load_raw),
                self.0 as _,
            );
        }
    }

    #[inline]
    fn normalize<'js>(
        opaque: &mut LoaderOpaque,
        ctx: Ctx<'js>,
        base: &CStr,
        name: &CStr,
    ) -> Result<*mut qjs::c_char> {
        let base = base.to_str()?;
        let name = name.to_str()?;

        let name = opaque.resolver.resolve(ctx, base, name)?;

        // We should transfer ownership of this string to QuickJS
        Ok(
            unsafe {
                qjs::js_strndup(ctx.as_ptr(), name.as_ptr() as _, name.as_bytes().len() as _)
            },
        )
    }

    unsafe extern "C" fn normalize_raw(
        ctx: *mut qjs::JSContext,
        base: *const qjs::c_char,
        name: *const qjs::c_char,
        opaque: *mut qjs::c_void,
    ) -> *mut qjs::c_char {
        let ctx = Ctx::from_ptr(ctx);
        let base = CStr::from_ptr(base);
        let name = CStr::from_ptr(name);
        let loader = &mut *(opaque as *mut LoaderOpaque);

        Self::normalize(loader, ctx, base, name).unwrap_or_else(|error| {
            error.throw(ctx);
            ptr::null_mut()
        })
    }

    #[inline]
    unsafe fn load<'js>(
        opaque: &mut LoaderOpaque,
        ctx: Ctx<'js>,
        name: &CStr,
    ) -> Result<*mut qjs::JSModuleDef> {
        let name = name.to_str()?;

        Ok(opaque
            .loader
            .load(ctx, name)?
            .define(ctx)?
            .as_module_def()
            .as_ptr())
    }

    unsafe extern "C" fn load_raw(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
        opaque: *mut qjs::c_void,
    ) -> *mut qjs::JSModuleDef {
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name);
        let loader = &mut *(opaque as *mut LoaderOpaque);

        Self::load(loader, ctx, name).unwrap_or_else(|error| {
            error.throw(ctx);
            ptr::null_mut()
        })
    }
}
