use crate::{qjs, Ctx, Error, Module, Result};
use relative_path::{RelativePath, RelativePathBuf};
use std::{ffi::CStr, ptr};

/// Module resolver trait
///
/// # Features
/// This trait is only availble if the `loader` feature is enabled.
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
    ///     format!("{}/{}", path, name)
    /// })
    /// # }
    /// ```
    fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String>;
}

/// Module loader trait
///
/// # Features
/// This trait is only availble if the `loader` feature is enabled.
pub trait Loader {
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

        let name = opaque.resolver.resolve(ctx, &base, &name)?;

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
        let loader = &mut *(opaque as *mut LoaderOpaque);

        Self::normalize(loader, ctx, &base, &name).unwrap_or_else(|_| ptr::null_mut())
    }

    #[inline]
    fn load<'js>(
        opaque: &mut LoaderOpaque,
        ctx: Ctx<'js>,
        name: &CStr,
    ) -> Result<*mut qjs::JSModuleDef> {
        let name = name.to_str()?;

        Ok(opaque.loader.load(ctx, name)?.as_module_def())
    }

    unsafe extern "C" fn load_raw(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
        opaque: *mut qjs::c_void,
    ) -> *mut qjs::JSModuleDef {
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name);
        let loader = &mut *(opaque as *mut LoaderOpaque);

        Self::load(loader, ctx, &name).unwrap_or_else(|_| ptr::null_mut())
    }
}

/// The file module resolver
///
/// This resolver can be used as the nested backing resolver in user-defined resolvers.
pub struct FileResolver {
    paths: Vec<RelativePathBuf>,
    extensions: Vec<String>,
}

impl FileResolver {
    pub fn add_path<P: AsRef<str>>(&mut self, path: P) -> &mut Self {
        self.paths.push(path.as_ref().into());
        self
    }

    pub fn add_extension<X: AsRef<str>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.as_ref().into());
        self
    }

    fn try_extensions(&self, path: &RelativePath) -> Option<RelativePathBuf> {
        if let Some(extension) = &path.extension() {
            if !is_file(path) {
                return None;
            }
            // check for known extensions
            self.extensions
                .iter()
                .find(|known_extension| known_extension == extension)
                .map(|_| path.to_relative_path_buf())
        } else {
            // try add any known extensions
            self.extensions
                .iter()
                .filter_map(|extension| {
                    let file = path.with_extension(extension);
                    if is_file(&file) {
                        Some(file)
                    } else {
                        None
                    }
                })
                .next()
        }
    }
}

impl Default for FileResolver {
    fn default() -> Self {
        Self {
            paths: vec![],
            extensions: vec!["js".into()],
        }
    }
}

impl Resolver for FileResolver {
    fn resolve<'js>(&mut self, _ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
        let path = if !name.starts_with('.') {
            self.paths
                .iter()
                .filter_map(|path| {
                    let path = path.join_normalized(name);
                    self.try_extensions(&path)
                })
                .next()
                .ok_or(Error::Unknown)?
        } else {
            let path = RelativePath::new(base);
            let path = if let Some(dir) = path.parent() {
                dir.join_normalized(name)
            } else {
                name.into()
            };
            self.try_extensions(&path).ok_or(Error::Unknown)?
        };
        Ok(path.to_string())
    }
}

/// The script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[derive(Clone, Debug)]
pub struct ScriptLoader {
    extensions: Vec<String>,
}

impl ScriptLoader {
    pub fn add_extension<X: AsRef<str>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.as_ref().into());
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
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js>> {
        check_extensions(&path, &self.extensions)?;

        let source: Vec<_> = std::fs::read(&path)?;
        ctx.compile(path, source)
    }
}

#[cfg(feature = "dyn-load")]
/// The native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
///
/// # Features
/// This struct is only available if the `dyn-load` features is enabled.
#[derive(Clone, Debug)]
pub struct NativeLoader {
    extensions: Vec<String>,
}

#[cfg(feature = "dyn-load")]
impl NativeLoader {
    pub fn add_extension<X: AsRef<str>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.as_ref().into());
        self
    }
}

#[cfg(feature = "dyn-load")]
impl Default for NativeLoader {
    fn default() -> Self {
        Self {
            extensions: vec![
                #[cfg(target_family = "windows")]
                {
                    "dll".into()
                },
                #[cfg(all(target_family = "unix"))]
                {
                    "so".into()
                },
                #[cfg(target_vendor = "apple")]
                {
                    "dylib".into()
                },
            ],
        }
    }
}

#[cfg(feature = "dyn-load")]
impl Loader for NativeLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js>> {
        use dlopen::raw::Library;
        use std::ffi::CString;

        check_extensions(&path, &self.extensions)?;

        type LoadFn =
            unsafe extern "C" fn(*mut qjs::JSContext, *const qjs::c_char) -> *mut qjs::JSModuleDef;

        let lib = Library::open(&path).map_err(|_| Error::Unknown)?;
        let load_fn: LoadFn =
            unsafe { lib.symbol("js_init_module") }.map_err(|_| Error::Unknown)?;

        let name = CString::new(path)?;
        let ptr = unsafe { load_fn(ctx.ctx, name.as_ptr()) };

        if ptr.is_null() {
            Err(Error::Unknown)
        } else {
            Ok(unsafe { Module::from_module_def(ctx, ptr) })
        }
    }
}

fn is_file<P: AsRef<RelativePath>>(path: P) -> bool {
    path.as_ref().to_path(".").is_file()
}

fn check_extensions(name: &str, extensions: &[String]) -> Result<()> {
    let path = RelativePath::new(name);
    let extension = path.extension().ok_or(Error::Unknown)?;
    let _ = extensions
        .iter()
        .find(|known_extension| known_extension == &extension)
        .ok_or(Error::Unknown)?;
    Ok(())
}

macro_rules! loader_impls {
    ($($($t:ident)*,)*) => {
        $(
            impl<$($t,)*> Resolver for ($($t,)*)
            where
                $($t: Resolver,)*
            {
                #[allow(non_snake_case)]
                fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
                    let ($($t,)*) = self;
                    $(
                        if let Ok(name) = $t.resolve(ctx, base, name) {
                            return Ok(name);
                        }
                    )*
                        Err(Error::Unknown)
                }
            }

            impl<$($t,)*> Loader for ($($t,)*)
            where
                $($t: Loader,)*
            {
                #[allow(non_snake_case)]
                fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js>> {
                    let ($($t,)*) = self;
                    $(
                        if let Ok(name) = $t.load(ctx, name) {
                            return Ok(name);
                        }
                    )*
                    Err(Error::Unknown)
                }
            }
        )*
    };
}

loader_impls! {
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn user_loader() {
        struct TestResolver;

        impl Resolver for TestResolver {
            fn resolve<'js>(
                &mut self,
                _ctx: Ctx<'js>,
                base: &str,
                name: &str,
            ) -> Result<StdString> {
                assert_eq!(base, "test_loader");
                assert_eq!(name, "test");
                Ok(name.into())
            }
        }

        struct TestLoader;

        impl Loader for TestLoader {
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
        rt.set_loader(TestResolver, TestLoader);
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
