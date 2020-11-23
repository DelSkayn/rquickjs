use crate::{qjs, Ctx, Error, Module, Result};
use std::{
    ffi::CStr,
    fs::read,
    path::{Path, PathBuf},
    ptr,
};

/// Module loader trait
///
/// # Features
/// This trait is only availble if the `loader` feature is enabled.
pub trait Loader {
    /// Normalize module name
    ///
    /// The default normalization looks like:
    ///
    /// ```no_run
    /// # use std::path::{Path, PathBuf};
    /// # use rquickjs::{Ctx, Result, Error};
    /// # fn default_normalize<'js>(_ctx: Ctx<'js>, base: &Path, name: &Path) -> Result<PathBuf> {
    /// Ok(if !name.starts_with(".") {
    ///     name.into()
    /// } else {
    ///     base.parent()
    ///         .ok_or(Error::Unknown)?
    ///         .join(name)
    ///         .canonicalize()?
    /// })
    /// # }
    /// ```
    fn normalize<'js>(&mut self, ctx: Ctx<'js>, base: &Path, name: &Path) -> Result<PathBuf> {
        default_normalize(ctx, base, name)
    }

    /// Load module by name
    ///
    /// The example loading may looks like:
    ///
    /// ```no_run
    /// # use std::{fs::read, path::Path};
    /// # use rquickjs::{Ctx, Module, Result};
    /// # fn default_load<'js>(ctx: Ctx<'js>, path: &Path) -> Result<Module<'js>> {
    /// let name = path.to_string_lossy();
    /// let source: Vec<_> = read(path)?;
    /// ctx.compile(name.as_ref(), source)
    /// # }
    /// ```
    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &Path) -> Result<Module<'js>> {
        default_load(ctx, name)
    }
}

fn default_normalize<'js>(_ctx: Ctx<'js>, base: &Path, name: &Path) -> Result<PathBuf> {
    Ok(if !name.starts_with(".") {
        name.into()
    } else {
        base.parent()
            .ok_or(Error::Unknown)?
            .join(name)
            .canonicalize()?
    })
}

fn default_load<'js>(ctx: Ctx<'js>, path: &Path) -> Result<Module<'js>> {
    let name = path.to_string_lossy();
    let source: Vec<_> = read(path)?;
    ctx.compile(name.as_ref(), source)
}

type DynLoader = Box<dyn Loader>;

#[repr(transparent)]
pub(crate) struct LoaderHolder(*mut DynLoader);

impl Drop for LoaderHolder {
    fn drop(&mut self) {
        let _loader = unsafe { Box::from_raw(self.0) };
    }
}

impl LoaderHolder {
    pub fn new<L>(loader: L) -> Self
    where
        L: Loader + 'static,
    {
        Self(Box::into_raw(Box::new(Box::new(loader))))
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
        loader: &mut DynLoader,
        ctx: Ctx<'js>,
        base: &CStr,
        name: &CStr,
    ) -> Result<*mut qjs::c_char> {
        let base = Path::new(base.to_str()?);
        let name = Path::new(name.to_str()?);

        let name = loader.normalize(ctx, &base, &name)?;
        let name = name.to_string_lossy();

        // We should transfer ownership of this string to QuickJS
        Ok(unsafe { qjs::js_strndup(ctx.ctx, name.as_ptr() as _, name.as_bytes().len() as _) })
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
        let loader = &mut *(opaque as *mut DynLoader);

        Self::normalize(loader, ctx, &base, &name).unwrap_or_else(|_| ptr::null_mut())
    }

    #[inline]
    fn load<'js>(
        loader: &mut DynLoader,
        ctx: Ctx<'js>,
        name: &CStr,
    ) -> Result<*mut qjs::JSModuleDef> {
        let name = Path::new(name.to_str()?);

        Ok(loader.load(ctx, name)?.as_module_def())
    }

    unsafe extern "C" fn load_raw(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
        opaque: *mut qjs::c_void,
    ) -> *mut qjs::JSModuleDef {
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name);
        let loader = &mut *(opaque as *mut DynLoader);

        Self::load(loader, ctx, &name).unwrap_or_else(|_| ptr::null_mut())
    }
}
