//! Integration-test-only observation hooks for the JIT lifecycle.

use super::Runtime;

pub fn set_runtime_drop_probe<F>(runtime: &Runtime, probe: F)
where
    F: FnOnce() + Send + 'static,
{
    runtime.inner.lock().set_jit_runtime_drop_probe(probe);
}

pub const fn fresh_bindgen_bindings() -> Option<&'static str> {
    crate::qjs::JIT_BINDGEN_BINDINGS
}
