use crate::{qjs, Ctx, Error, Module, Result};
use std::{ffi::CStr, ptr};

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
    /// # use rquickjs::{Ctx, Result, Error};
    /// # fn default_normalize<'js>(_ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
    /// Ok(if !name.starts_with('.') {
    ///     name.into()
    /// } else {
    ///     let mut split = base.rsplitn(2, '/');
    ///     let path = match (split.next(), split.next()) {
    ///         (_, Some(path)) => path,
    ///         _ => "",
    ///     };
    ///     format!("{}/{}", path, name)
    /// })
    /// # }
    /// ```
    fn normalize<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String>;

    /// Load module by name
    ///
    /// The example loading may looks like:
    ///
    /// ```no_run
    /// # use rquickjs::{Ctx, Module, Result};
    /// # fn default_load<'js>(ctx: Ctx<'js>, name: &str) -> Result<Module<'js>> {
    /// let path = std::path::Path::new(name);
    /// let path = if path.extension().is_none() {
    ///     path.with_extension("js")
    /// } else {
    ///     path.into()
    /// };
    /// let source: Vec<_> = std::fs::read(path)?;
    /// ctx.compile(name, source)
    /// # }
    /// ```
    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js>>;
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
        let base = base.to_str()?;
        let name = name.to_str()?;

        let name = loader.normalize(ctx, &base, &name)?;

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
        let name = name.to_str()?;

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

/// The default module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[derive(Clone, Debug)]
pub struct DefaultLoader {
    pub extensions: Vec<(String, ModuleType)>,
}

/// The type of module known by default loader
#[derive(Debug, Clone, Copy)]
pub enum ModuleType {
    Script,
    #[cfg(feature = "dyn-load")]
    Native,
}

impl DefaultLoader {
    pub fn add_extension<X: Into<String>>(
        &mut self,
        extension: X,
        module_type: ModuleType,
    ) -> &mut Self {
        self.extensions.push((extension.into(), module_type));
        self
    }
}

impl Loader for DefaultLoader {
    fn normalize<'js>(&mut self, _ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
        Ok(if !name.starts_with('.') {
            name.into()
        } else {
            let mut split = base.rsplitn(2, '/');
            let path = match (split.next(), split.next()) {
                (_, Some(path)) => path,
                _ => "",
            };
            format!("{}/{}", path, name)
        })
    }

    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js>> {
        use std::{ffi::OsStr, path::Path};

        let path = Path::new(name);
        let (path, module_type) = if let Some(extension) = &path.extension() {
            if !path.is_file() {
                return Err(Error::Unknown);
            }
            let (_, module_kind) = self
                .extensions
                .iter()
                .find(|(known_extension, _)| &OsStr::new(known_extension) == extension)
                .ok_or(Error::Unknown)?;
            (path.into(), module_kind)
        } else {
            self.extensions
                .iter()
                .filter_map(|(extension, module_kind)| {
                    let file = path.with_extension(extension);
                    if file.is_file() {
                        Some((file, module_kind))
                    } else {
                        None
                    }
                })
                .next()
                .ok_or(Error::Unknown)?
        };

        match module_type {
            ModuleType::Script => {
                let source: Vec<_> = std::fs::read(path)?;
                ctx.compile(name, source)
            }
            #[cfg(feature = "dyn-load")]
            ModuleType::Native => {
                use dlopen::raw::Library;

                type LoadFn = unsafe extern "C" fn(
                    *mut qjs::JSContext,
                    *const qjs::c_char,
                ) -> *mut qjs::JSModuleDef;

                let lib = Library::open(path)?;
                let load_fn: LoadFn = unsafe { lib.symbol("js_init_module") }?;

                let name = CString::new(name);
                let module = unsafe { load_fn(ctx.ctx, name, name.as_ptr()) };

                Ok(Module::from_module_def(module))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn user_loader() {
        struct TestLoader;
        impl Loader for TestLoader {
            fn normalize<'js>(
                &mut self,
                _ctx: Ctx<'js>,
                base: &str,
                name: &str,
            ) -> Result<StdString> {
                assert_eq!(base, "test_loader");
                assert_eq!(name, "test");
                Ok(name.into())
            }

            fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js>> {
                assert_eq!(name, "test");
                ctx.compile(
                    "test",
                    r#"
                      export const n = 123;
                      export const s = "abc";
                    "#,
                )
            }
        }

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        rt.set_loader(TestLoader);
        ctx.with(|ctx| {
            eprintln!("test");
            let _module = ctx
                .compile(
                    "test_loader",
                    r#"
                      import { n, s } from "test";
                      export default [n, s];
                    "#,
                )
                .unwrap();
        })
    }
}
