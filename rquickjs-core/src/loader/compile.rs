use super::resolve_simple;
use crate::{Ctx, Loaded, Loader, Module, Resolver, Result, SafeRef, SafeRefGuard, Script};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

/// Modules compiling data
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Default, Clone)]
pub struct Compile<T = ()> {
    data: SafeRef<CompileData>,
    inner: T,
}

impl<T> Deref for Compile<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Compile<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Compile {
    /// Create new compiling scope
    pub fn new() -> Self {
        Self::default()
    }

    /// Create compiling resolver by wrapping other resolver
    pub fn resolver<R: Resolver>(&self, resolver: R) -> Compile<R> {
        Compile {
            data: self.data.clone(),
            inner: resolver,
        }
    }

    /// Create compiling loader by wrapping other script loader
    pub fn loader<L: Loader<Script>>(&self, loader: L) -> Compile<L> {
        Compile {
            data: self.data.clone(),
            inner: loader,
        }
    }

    /// Iterator over compiled scripts
    pub fn modules(&self) -> CompileModules {
        CompileModules(self.data.lock())
    }
}

pub struct CompileModules<'i>(SafeRefGuard<'i, CompileData>);

impl<'i, 'r: 'i> IntoIterator for &'r CompileModules<'i> {
    type IntoIter = CompileDataIter<'i>;
    type Item = (&'i str, &'i [u8]);
    fn into_iter(self) -> Self::IntoIter {
        CompileDataIter {
            data: &*self.0,
            index: 0,
        }
    }
}

pub struct CompileDataIter<'r> {
    data: &'r CompileData,
    index: usize,
}

impl<'i> Iterator for CompileDataIter<'i> {
    type Item = (&'i str, &'i [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let CompileData { modules, bytecodes } = &self.data;
        if self.index < bytecodes.len() {
            let (path, data) = &bytecodes[self.index];
            self.index += 1;
            modules
                .get(path.as_str())
                .map(|name| (name.as_str(), data.as_ref()))
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
struct CompileData {
    // { module_path: internal_name }
    modules: HashMap<String, String>,
    // [ (module_path, module_bytecode) ]
    bytecodes: Vec<(String, Vec<u8>)>,
}

impl<R> Resolver for Compile<R>
where
    R: Resolver,
{
    fn resolve<'js>(&mut self, ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
        self.inner.resolve(ctx, base, name).map(|path| {
            let name = resolve_simple(base, name);
            self.data.lock().modules.insert(path.clone(), name);
            path
        })
    }
}

impl<L> Loader<Script> for Compile<L>
where
    L: Loader<Script>,
{
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, Loaded<Script>>> {
        self.inner.load(ctx, path).and_then(|module| {
            self.data
                .lock()
                .bytecodes
                .push((path.into(), module.write_object(false)?));
            Ok(module)
        })
    }
}
