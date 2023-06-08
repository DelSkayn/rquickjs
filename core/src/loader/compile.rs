use crate::{
    loader::{util::resolve_simple, Loader, RawLoader, Resolver},
    module::ModuleDataKind,
    Ctx, Lock, Module, Mut, Ref, Result,
};
use std::{
    collections::{hash_map::Iter as HashMapIter, HashMap},
    iter::{ExactSizeIterator, FusedIterator},
    ops::{Deref, DerefMut},
};

/// Modules compiling data
#[derive(Default, Clone)]
pub struct Compile<T = ()> {
    data: Ref<Mut<CompileData>>,
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
    pub fn loader<L: Loader>(&self, loader: L) -> Compile<L> {
        Compile {
            data: self.data.clone(),
            inner: loader,
        }
    }

    /// Get resolved modules with paths
    ///
    /// You can use [`IntoIterator::into_iter()`] to get an iterator over tuples which includes module _name_ (`&str`) and _path_ (`&str`).
    pub fn modules(&self) -> ResolvedModules {
        ResolvedModules(self.data.lock())
    }

    /// Get loaded modules with bytecodes
    ///
    /// You can use [`IntoIterator::into_iter()`] to get an iterator over tuples which includes module _path_ (`&str`) and _bytecode_ (`&[u8]`).
    pub fn bytecodes(&self) -> CompiledBytecodes {
        CompiledBytecodes(self.data.lock())
    }
}

/// A list of resolved modules
///
/// It can be converted into iterator over resolved modules.
pub struct ResolvedModules<'i>(Lock<'i, CompileData>);

impl<'i, 'r: 'i> IntoIterator for &'r ResolvedModules<'i> {
    type IntoIter = ResolvedModulesIter<'i>;
    type Item = (&'i str, &'i str);
    fn into_iter(self) -> Self::IntoIter {
        ResolvedModulesIter(self.0.modules.iter())
    }
}

/// An iterator over resolved modules
///
/// Each item is a tuple consists of module name and path.
pub struct ResolvedModulesIter<'r>(HashMapIter<'r, String, String>);

impl<'i> Iterator for ResolvedModulesIter<'i> {
    type Item = (&'i str, &'i str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(path, name)| (name.as_str(), path.as_str()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'i> ExactSizeIterator for ResolvedModulesIter<'i> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'i> FusedIterator for ResolvedModulesIter<'i> {}

/// A list of compiled bytecodes of loaded modules
///
/// It can be converted into iterator of loaded modules with bytecodes.
pub struct CompiledBytecodes<'i>(Lock<'i, CompileData>);

impl<'i, 'r: 'i> IntoIterator for &'r CompiledBytecodes<'i> {
    type IntoIter = CompiledBytecodesIter<'i>;
    type Item = (&'i str, &'i [u8]);
    fn into_iter(self) -> Self::IntoIter {
        CompiledBytecodesIter {
            data: &self.0,
            index: 0,
        }
    }
}

/// An iterator over loaded bytecodes of modules
///
/// Each item is a tuple of module path and bytecode.
pub struct CompiledBytecodesIter<'r> {
    data: &'r CompileData,
    index: usize,
}

impl<'i> Iterator for CompiledBytecodesIter<'i> {
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'i> ExactSizeIterator for CompiledBytecodesIter<'i> {
    fn len(&self) -> usize {
        self.data.bytecodes.len() - self.index
    }
}

impl<'i> FusedIterator for CompiledBytecodesIter<'i> {}

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

unsafe impl<L> RawLoader for Compile<L>
where
    L: Loader,
{
    unsafe fn raw_load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js>> {
        let data = self.inner.load(ctx, path)?;
        assert!(
            matches!(data.kind(), ModuleDataKind::Source(_) | ModuleDataKind::ByteCode(_)) ,
            "can't compile native modules, loader `{}` returned a native module, but `Compile` can only handle modules loaded from source or bytecode",
            std::any::type_name::<L>()
        );
        let module = data.unsafe_declare(ctx)?;
        let data = module.write_object(false)?;
        self.data.lock().bytecodes.push((path.into(), data));
        Ok(module)
    }
}
