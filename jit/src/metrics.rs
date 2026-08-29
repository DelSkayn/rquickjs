//! Runtime-visible JIT metrics.

/// Immutable metrics exposed by the no-op JIT backend.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JitMetrics {
    native_enabled: bool,
}

impl JitMetrics {
    pub(crate) const fn disabled() -> Self {
        Self {
            native_enabled: false,
        }
    }

    /// Reports whether this runtime can currently enter native JIT code.
    pub const fn native_enabled(&self) -> bool {
        self.native_enabled
    }
}
