use crate::{Ctx, Error, Resolver, Result};
use relative_path::RelativePath;
use std::collections::HashSet;

/// The builtin module resolver
///
/// This resolver can also be used as the nested backing resolver in user-defined resolvers.
#[derive(Debug)]
pub struct BuiltinResolver {
    modules: HashSet<String>,
}

impl BuiltinResolver {
    /// Add builtin module
    pub fn add_module<P: Into<String>>(&mut self, path: P) -> &mut Self {
        self.modules.insert(path.into());
        self
    }

    /// Add builtin module
    pub fn with_module<P: Into<String>>(mut self, path: P) -> Self {
        self.add_module(path);
        self
    }
}

impl Default for BuiltinResolver {
    fn default() -> Self {
        Self {
            modules: HashSet::new(),
        }
    }
}

impl Resolver for BuiltinResolver {
    fn resolve<'js>(&mut self, _ctx: Ctx<'js>, base: &str, name: &str) -> Result<String> {
        let full = if !name.starts_with('.') {
            name.to_string()
        } else {
            let base = RelativePath::new(base);
            if let Some(dir) = base.parent() {
                dir.join_normalized(name).to_string()
            } else {
                name.to_string()
            }
        };

        if self.modules.contains(&full) {
            Ok(full)
        } else {
            Err(Error::new_resolving(base, name))
        }
    }
}
