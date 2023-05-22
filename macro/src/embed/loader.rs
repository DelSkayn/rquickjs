use std::{
    collections::HashMap,
    iter::{ExactSizeIterator, FusedIterator},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, MutexGuard},
};

use rquickjs_core::{
    loader::{util::resolve_simple, Loader, RawLoader, Resolver},
    Ctx, Module, ModuleData, ModuleDataKind, Result,
};

/// Modules compiling data
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Default, Clone)]
pub struct Embed<T = ()> {
    data: Arc<Mutex<CompileData>>,
    inner: T,
}

impl<T> Deref for Embed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Embed<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Embed {
    /// Create new compiling scope
    pub fn new() -> Self {
        Self::default()
    }

    /// Create compiling resolver by wrapping other resolver
    pub fn resolver<R: Resolver>(&self, resolver: R) -> Embed<R> {
        Embed {
            data: self.data.clone(),
            inner: resolver,
        }
    }

    /// Create compiling loader by wrapping other script loader
    pub fn loader<L: Loader>(&self, loader: L) -> Embed<L> {
        Embed {
            data: self.data.clone(),
            inner: loader,
        }
    }

    /// Get loaded modules with bytecodes
    ///
    /// You can use [`IntoIterator::into_iter()`] to get an iterator over tuples which includes module _path_ (`&str`) and _bytecode_ (`&[u8]`).
    pub fn bytecodes(&self) -> CompiledBytecodes {
        CompiledBytecodes(self.data.lock().unwrap())
    }
}

/// A list of compiled bytecodes of loaded modules
///
/// It can be converted into iterator of loaded modules with bytecodes.
pub struct CompiledBytecodes<'i>(MutexGuard<'i, CompileData>);

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
        let (path, data) = self.data.bytecodes.get(self.index)?;
        self.index += 1;
        Some((path.as_ref(), data.as_ref()))
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

impl<R> Resolver for Embed<R>
where
    R: Resolver,
{
    fn resolve(&mut self, ctx: Ctx, base: &str, name: &str) -> Result<String> {
        // Ignore base because it is always the same.
        self.inner.resolve(ctx, base, name).map(|path| {
            let name = resolve_simple(base, name);
            self.data.lock().unwrap().modules.insert(name.clone(), path);
            name
        })
    }
}

unsafe impl<L> RawLoader for Embed<L>
where
    L: Loader,
{
    unsafe fn raw_load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js>> {
        let load_path = self
            .data
            .lock()
            .unwrap()
            .modules
            .get(path)
            .unwrap()
            .to_owned();
        let data = self.inner.load(ctx, &load_path)?;
        let data = match data.kind() {
            ModuleDataKind::Source(x) => x.clone(),
            _ => {
                return Err(rquickjs_core::Error::Loading {
                    name: path.to_string(),
                    message: Some("could not embed a non source module".to_string()),
                })
            }
        };

        let module = ModuleData::source(path, data).unsafe_declare(ctx)?;
        let data = module.write_object(false)?;
        self.data
            .lock()
            .unwrap()
            .bytecodes
            .push((path.into(), data));
        Ok(module)
    }
}
