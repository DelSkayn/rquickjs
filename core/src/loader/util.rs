use alloc::string::{String, ToString as _};
use relative_path::RelativePath;

pub fn resolve_simple(base: &str, name: &str) -> String {
    if name.starts_with('.') {
        let path = RelativePath::new(base);
        if let Some(dir) = path.parent() {
            return dir.join_normalized(name).to_string();
        }
    }
    name.into()
}

#[allow(dead_code)] // not used in no_std
pub fn check_extensions(name: &str, extensions: &[String]) -> bool {
    let path = RelativePath::new(name);
    path.extension()
        .map(|extension| {
            extensions
                .iter()
                .any(|known_extension| known_extension == extension)
        })
        .unwrap_or(false)
}
