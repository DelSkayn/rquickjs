use crate::{qjs, BeforeInit, Ctx, Error, Module, Result};
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
    fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js, BeforeInit>>;
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

        Self::normalize(loader, ctx, &base, &name).unwrap_or_else(|error| {
            error.throw(ctx);
            ptr::null_mut()
        })
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

        Self::load(loader, ctx, &name).unwrap_or_else(|error| {
            error.throw(ctx);
            ptr::null_mut()
        })
    }
}

/// The file module resolver
///
/// This resolver can be used as the nested backing resolver in user-defined resolvers.
#[derive(Debug)]
pub struct FileResolver {
    paths: Vec<RelativePathBuf>,
    patterns: Vec<String>,
}

impl FileResolver {
    /// Add search path for modules
    pub fn add_path<P: Into<RelativePathBuf>>(&mut self, path: P) -> &mut Self {
        self.paths.push(path.into());
        self
    }

    /// Add module file pattern
    pub fn add_pattern<P: Into<String>>(&mut self, pattern: P) -> &mut Self {
        self.patterns.push(pattern.into());
        self
    }

    /// Add support for native modules
    pub fn add_native(&mut self) -> &mut Self {
        #[cfg(target_family = "windows")]
        self.add_pattern("{}.dll");

        #[cfg(target_vendor = "apple")]
        self.add_pattern("{}.dylib").add_pattern("lib{}.dylib");

        #[cfg(all(target_family = "unix"))]
        self.add_pattern("{}.so").add_pattern("lib{}.so");

        self
    }

    /// Build resolver
    pub fn build(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            patterns: self.patterns.clone(),
        }
    }

    fn try_patterns(&self, path: &RelativePath) -> Option<RelativePathBuf> {
        if let Some(extension) = &path.extension() {
            if !is_file(path) {
                return None;
            }
            // check for known extensions
            self.patterns
                .iter()
                .find(|pattern| {
                    let path = RelativePath::new(pattern);
                    if let Some(known_extension) = &path.extension() {
                        known_extension == extension
                    } else {
                        false
                    }
                })
                .map(|_| path.to_relative_path_buf())
        } else {
            // try with known patterns
            self.patterns
                .iter()
                .filter_map(|pattern| {
                    let name = pattern.replace("{}", path.file_name()?);
                    let file = path.with_file_name(&name);
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
            patterns: vec!["{}.js".into()],
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
                    self.try_patterns(&path)
                })
                .next()
        } else {
            let path = RelativePath::new(base);
            let path = if let Some(dir) = path.parent() {
                dir.join_normalized(name)
            } else {
                name.into()
            };
            self.try_patterns(&path)
        }
        .ok_or_else(|| Error::resolving::<_, _, &str>(base, name, None))?;

        Ok(path.to_string())
    }
}

/// The script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[derive(Debug)]
pub struct ScriptLoader {
    extensions: Vec<String>,
}

impl ScriptLoader {
    /// Add file extensions
    pub fn add_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.into());
        self
    }

    /// Build loader
    pub fn build(&self) -> Self {
        Self {
            extensions: self.extensions.clone(),
        }
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
            return Err(Error::loading::<_, &str>(path, None));
        }

        let source: Vec<_> = std::fs::read(&path)?;
        ctx.compile_only(path, source)
    }
}

#[cfg(feature = "dyn-load")]
/// The native module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
///
/// # Features
/// This struct is only available if the `dyn-load` features is enabled.
#[derive(Debug)]
pub struct NativeLoader {
    extensions: Vec<String>,
    libs: Vec<dlopen::raw::Library>,
}

#[cfg(feature = "dyn-load")]
impl NativeLoader {
    /// Add file extension
    pub fn add_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.into());
        self
    }

    /// Build loader
    pub fn build(&self) -> Self {
        Self {
            extensions: self.extensions.clone(),
            libs: Vec::new(),
        }
    }
}

#[cfg(feature = "dyn-load")]
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

#[cfg(feature = "dyn-load")]
impl Loader for NativeLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, BeforeInit>> {
        use dlopen::raw::Library;
        use std::ffi::CString;

        if !check_extensions(&path, &self.extensions) {
            return Err(Error::loading::<_, &str>(path, None));
        }

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
            self.libs.push(lib);
            Ok(unsafe { Module::from_module_def(ctx, ptr) })
        }
    }
}

fn is_file<P: AsRef<RelativePath>>(path: P) -> bool {
    path.as_ref().to_path(".").is_file()
}

fn check_extensions(name: &str, extensions: &[String]) -> bool {
    let path = RelativePath::new(name);
    path.extension()
        .map(|extension| {
            extensions
                .iter()
                .any(|known_extension| known_extension == extension)
        })
        .unwrap_or(false)
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
                    let mut messages = Vec::new();
                    let ($($t,)*) = self;
                    $(
                        match $t.resolve(ctx, base, name) {
                            // Still could try the next resolver
                            Err(Error::Resolving { message, .. }) => {
                                message.map(|message| messages.push(message));
                            },
                            result => return result,
                        }
                    )*
                    // Unable to resolve module name
                    let message = if messages.is_empty() { None } else {
                        Some(messages.join("\n"))
                    };
                    Err(Error::resolving(base, name, message))
                }
            }

            impl<$($t,)*> Loader for ($($t,)*)
            where
                $($t: Loader,)*
            {
                #[allow(non_snake_case)]
                fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js, BeforeInit>> {
                    let mut messages = Vec::new();
                    let ($($t,)*) = self;
                    $(
                        match $t.load(ctx, name) {
                            // Still could try the next loader
                            Err(Error::Loading { message, .. }) => {
                                message.map(|message| messages.push(message));
                            },
                            result => return result,
                        }
                    )*
                    // Unable to load module
                    let message = if messages.is_empty() { None } else {
                        Some(messages.join("\n"))
                    };
                    Err(Error::loading(name, message))
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

    struct TestResolver;

    impl Resolver for TestResolver {
        fn resolve<'js>(&mut self, _ctx: Ctx<'js>, base: &str, name: &str) -> Result<StdString> {
            if base == "loader" && name == "test" {
                Ok(name.into())
            } else {
                Err(Error::resolving(base, name, Some("unable to resolve")))
            }
        }
    }

    struct TestLoader;

    impl Loader for TestLoader {
        fn load<'js>(&mut self, ctx: Ctx<'js>, name: &str) -> Result<Module<'js, BeforeInit>> {
            if name == "test" {
                ctx.compile_only(
                    "test",
                    r#"
                      export const n = 123;
                      export const s = "abc";
                    "#,
                )
            } else {
                Err(Error::loading(name, Some("unable to load")))
            }
        }
    }

    #[test]
    fn custom_loader() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        rt.set_loader(TestResolver, TestLoader);
        ctx.with(|ctx| {
            let _module = ctx
                .compile(
                    "loader",
                    r#"
                      import { n, s } from "test";
                      export default [n, s];
                    "#,
                )
                .unwrap();
        })
    }

    #[test]
    fn resolving_error() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        rt.set_loader(TestResolver, TestLoader);
        ctx.with(|ctx| {
            if let Err(error) = ctx.compile(
                "loader",
                r#"
                      import { n, s } from "test_";
                    "#,
            ) {
                assert_eq!(error.to_string(), "exception generated by quickjs: []:0 error resolving module \'test_\' with base \'loader\': unable to resolve\n");
                if let Error::Exception { stack, .. } = error {
                    // FIXME: This does not be empty
                    assert_eq!(stack, "");
                } else {
                    assert!(false);
                }
            } else {
                assert!(false);
            }
        })
    }
}
