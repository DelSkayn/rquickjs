//! Utilities for embedding JS modules.

use super::{util::resolve_simple, Loader, Resolver};
use crate::{Ctx, Error, Module, Result};
use alloc::string::String;
use core::ops::Deref;

/// The module data which contains bytecode
///
/// This trait needed because the modules potentially can contain any kind of data like a typing (for TypeScript) or metadata.
pub trait HasByteCode<'bc> {
    fn get_bytecode(&self) -> &'bc [u8];
}

impl<'bc> HasByteCode<'bc> for &'bc [u8] {
    fn get_bytecode(&self) -> &'bc [u8] {
        self
    }
}

/// The alias for compiled modules represented as a static const arrays
///
/// The element is a tuple of `(module_name, module_data)`.
pub type ScaBundleData<D> = &'static [(&'static str, D)];

#[cfg(feature = "phf")]
/// The alias for compiled modules represented as a perfect hash maps
///
/// The key is a module name and the value is a module data.
pub type PhfBundleData<D> = &'static phf::Map<&'static str, D>;

/// The resolver and loader for bundles of compiled modules
#[derive(Debug, Clone, Copy)]
pub struct Bundle<T>(pub T);

impl<T> Deref for Bundle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D> Resolver for Bundle<ScaBundleData<D>> {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
        let path = resolve_simple(base, name);
        if self.iter().any(|(name, _)| *name == path) {
            Ok(path)
        } else {
            Err(Error::new_resolving(base, name))
        }
    }
}

#[cfg(feature = "phf")]
impl<D> Resolver for Bundle<PhfBundleData<D>> {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
        let path = resolve_simple(base, name);
        if self.contains_key(path.as_str()) {
            Ok(path)
        } else {
            Err(Error::new_resolving(base, name))
        }
    }
}

impl<D> Loader for Bundle<ScaBundleData<D>>
where
    D: HasByteCode<'static>,
{
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js>> {
        if let Some((_, x)) = self.iter().find(|(module_name, _)| *module_name == name) {
            let module = unsafe { Module::load(ctx.clone(), x.get_bytecode())? };
            return Ok(module);
        }
        Err(Error::new_loading(name))
    }
}

#[cfg(feature = "phf")]
impl<D> Loader for Bundle<PhfBundleData<D>>
where
    D: HasByteCode<'static>,
{
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js>> {
        if let Some(x) = self.get(name) {
            let module = unsafe { Module::load(ctx.clone(), x.get_bytecode())? };
            return Ok(module);
        }
        Err(Error::new_loading(name))
    }
}
