use crate::{qjs, BeforeInit, Ctx, Error, Module, Result};
use relative_path::RelativePath;
use std::{ffi::CStr, ptr};

mod file_resolver;
pub use file_resolver::FileResolver;

mod script_loader;
pub use script_loader::ScriptLoader;

#[cfg(feature = "dyn-load")]
mod native_loader;
#[cfg(feature = "dyn-load")]
pub use native_loader::NativeLoader;

mod builtin_resolver;
pub use builtin_resolver::BuiltinResolver;

mod builtin_loader;
pub use builtin_loader::BuiltinLoader;

mod module_loader;
pub use module_loader::ModuleLoader;

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
                assert_eq!(error.to_string(), "Exception generated by quickjs: :0 Error resolving module \'test_\' from \'loader\': unable to resolve");
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
