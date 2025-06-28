//! Loaders and resolvers for loading JS modules.

use alloc::boxed::Box;
use alloc::string::String;
use core::{ffi::CStr, ptr};

use crate::{module::Declared, qjs, Ctx, Module, Result};

mod builtin_loader;
mod builtin_resolver;
pub mod bundle;
mod compile;
#[cfg(feature = "std")]
mod file_resolver;
mod module_loader;
mod script_loader;
mod util;

#[cfg(feature = "dyn-load")]
mod native_loader;

pub use builtin_loader::BuiltinLoader;
pub use builtin_resolver::BuiltinResolver;
pub use compile::Compile;
#[cfg(feature = "std")]
pub use file_resolver::FileResolver;
pub use module_loader::ModuleLoader;
pub use script_loader::ScriptLoader;

#[cfg(feature = "dyn-load")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "dyn-load")))]
pub use native_loader::NativeLoader;

#[cfg(feature = "phf")]
/// The type of bundle that the `embed!` macro returns
pub type Bundle = bundle::Bundle<bundle::PhfBundleData<&'static [u8]>>;

#[cfg(not(feature = "phf"))]
/// The type of bundle that the `embed!` macro returns
pub type Bundle = bundle::Bundle<bundle::ScaBundleData<&'static [u8]>>;

/// Module resolver interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub trait Resolver {
    /// Normalize module name
    ///
    /// The resolving may looks like:
    ///
    /// ```no_run
    /// # use rquickjs::{Ctx, Result, Error};
    /// # fn default_resolve<'js>(_ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
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
    fn resolve<'js>(&mut self, ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String>;
}

/// Module loader interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub trait Loader {
    /// Load module by name
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js, Declared>>;
}

struct LoaderOpaque {
    resolver: Box<dyn Resolver>,
    loader: Box<dyn Loader>,
}

#[derive(Debug)]
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
        ctx: &Ctx<'js>,
        base: &CStr,
        name: &CStr,
    ) -> Result<*mut qjs::c_char> {
        let base = base.to_str()?;
        let name = name.to_str()?;

        let name = opaque.resolver.resolve(ctx, base, name)?;

        // We should transfer ownership of this string to QuickJS
        Ok(unsafe { qjs::js_strndup(ctx.as_ptr(), name.as_ptr() as _, name.len() as _) })
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

        Self::normalize(loader, &ctx, base, name).unwrap_or_else(|error| {
            error.throw(&ctx);
            ptr::null_mut()
        })
    }

    #[inline]
    unsafe fn load<'js>(
        opaque: &mut LoaderOpaque,
        ctx: &Ctx<'js>,
        name: &CStr,
    ) -> Result<*mut qjs::JSModuleDef> {
        let name = name.to_str()?;

        Ok(opaque.loader.load(ctx, name)?.as_ptr())
    }

    unsafe extern "C" fn load_raw(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
        opaque: *mut qjs::c_void,
    ) -> *mut qjs::JSModuleDef {
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name);
        let loader = &mut *(opaque as *mut LoaderOpaque);

        Self::load(loader, &ctx, name).unwrap_or_else(|error| {
            error.throw(&ctx);
            ptr::null_mut()
        })
    }
}

macro_rules! loader_impls {
    ($($t:ident)*) => {
        loader_impls!(@sub @mark $($t)*);
    };
    (@sub $($lead:ident)* @mark $head:ident $($rest:ident)*) => {
        loader_impls!(@impl $($lead)*);
        loader_impls!(@sub $($lead)* $head @mark $($rest)*);
    };
    (@sub $($lead:ident)* @mark) => {
        loader_impls!(@impl $($lead)*);
    };
    (@impl $($t:ident)*) => {
            impl<$($t,)*> Resolver for ($($t,)*)
            where
                $($t: Resolver,)*
            {
                #[allow(non_snake_case)]
                #[allow(unused_mut)]
                fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
                    let mut messages = alloc::vec::Vec::<alloc::string::String>::new();
                    let ($($t,)*) = self;
                    $(
                        match $t.resolve(_ctx, base, name) {
                            // Still could try the next resolver
                            Err($crate::Error::Resolving { message, .. }) => {
                                message.map(|message| messages.push(message));
                            },
                            result => return result,
                        }
                    )*
                    // Unable to resolve module name
                    Err(if messages.is_empty() {
                        $crate::Error::new_resolving(base, name)
                    } else {
                        $crate::Error::new_resolving_message(base, name, messages.join("\n"))
                    })
                }
            }

            impl< $($t,)*> $crate::loader::Loader for ($($t,)*)
            where
                $($t: $crate::loader::Loader,)*
            {
                #[allow(non_snake_case)]
                #[allow(unused_mut)]
                fn load<'js>(&mut self, _ctx: &Ctx<'js>, name: &str) -> Result<Module<'js, Declared>> {
                    let mut messages = alloc::vec::Vec::<alloc::string::String>::new();
                    let ($($t,)*) = self;
                    $(
                        match $t.load(_ctx, name) {
                            // Still could try the next loader
                            Err($crate::Error::Loading { message, .. }) => {
                                message.map(|message| messages.push(message));
                            },
                            result => return result,
                        }
                    )*
                    // Unable to load module
                    Err(if messages.is_empty() {
                        $crate::Error::new_loading(name)
                    } else {
                        $crate::Error::new_loading_message(name, messages.join("\n"))
                    })
                }
            }
    };
}
loader_impls!(A B C D E F G H);

#[cfg(test)]
mod test {
    use crate::{CatchResultExt, Context, Ctx, Error, Module, Result, Runtime};

    use super::{Loader, Resolver};

    struct TestResolver;

    impl Resolver for TestResolver {
        fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
            if base == "loader" && name == "test" {
                Ok(name.into())
            } else {
                Err(Error::new_resolving_message(
                    base,
                    name,
                    "unable to resolve",
                ))
            }
        }
    }

    struct TestLoader;

    impl Loader for TestLoader {
        fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js>> {
            if name == "test" {
                Module::declare(
                    ctx.clone(),
                    "test",
                    r#"
                      export const n = 123;
                      export const s = "abc";
                    "#,
                )
            } else {
                Err(Error::new_loading_message(name, "unable to load"))
            }
        }
    }

    #[test]
    fn custom_loader() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        rt.set_loader(TestResolver, TestLoader);
        ctx.with(|ctx| {
            Module::evaluate(
                ctx,
                "loader",
                r#"
                      import { n, s } from "test";
                      export default [n, s];
                    "#,
            )
            .unwrap()
            .finish::<()>()
            .unwrap();
        })
    }

    #[test]
    #[should_panic(expected = "Error resolving module")]
    fn resolving_error() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        rt.set_loader(TestResolver, TestLoader);
        ctx.with(|ctx| {
            Module::evaluate(
                ctx.clone(),
                "loader",
                r#"
                      import { n, s } from "test_";
                    "#,
            )
            .catch(&ctx)
            .unwrap()
            .finish::<()>()
            .catch(&ctx)
            .expect("Unable to resolve");
        })
    }
}
