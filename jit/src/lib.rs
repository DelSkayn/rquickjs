//! Optional tiered JIT integration for `rquickjs`.
//!
//! This initial crate exposes the owning runtime API while keeping execution on
//! the QuickJS interpreter until the versioned engine ABI is available.

mod config;
mod error;
mod metrics;

use core::ops::Deref;

pub use config::{JitConfig, JitConfigBuilder};
pub use error::JitError;
pub use metrics::JitMetrics;
pub use rquickjs_core::Runtime;

/// Owns the guard that keeps a JIT backend attached to a runtime.
#[derive(Debug)]
pub struct Jit {
    metrics: JitMetrics,
}

impl Jit {
    /// Attaches the initial no-op backend to an existing runtime.
    pub fn attach(_runtime: &Runtime, _config: JitConfig) -> Result<Self, JitError> {
        Ok(Self {
            metrics: JitMetrics::disabled(),
        })
    }

    /// Fails when the current target cannot support native execution.
    #[cfg(target_arch = "wasm32")]
    pub fn require_native(&self) -> Result<(), JitError> {
        let _ = self;
        Err(JitError::UnsupportedPlatform)
    }

    /// Native execution is supported by this target, although this initial
    /// backend intentionally remains disabled.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn require_native(&self) -> Result<(), JitError> {
        Ok(())
    }

    /// Returns the metrics associated with this backend guard.
    pub const fn metrics(&self) -> &JitMetrics {
        &self.metrics
    }
}

/// Builder for an owning [`JitRuntime`].
#[derive(Clone, Debug, Default)]
pub struct JitRuntimeBuilder {
    config: JitConfig,
}

impl JitRuntimeBuilder {
    /// Sets the JIT policy and resource limits.
    pub fn config(mut self, config: JitConfig) -> Self {
        self.config = config;
        self
    }

    /// Constructs an interpreter runtime with a disabled JIT guard.
    pub fn build(self) -> Result<JitRuntime, JitError> {
        let runtime = Runtime::new()?;
        let jit = Jit::attach(&runtime, self.config)?;
        Ok(JitRuntime { runtime, jit })
    }
}

/// An owning QuickJS runtime paired with its JIT guard.
pub struct JitRuntime {
    runtime: Runtime,
    jit: Jit,
}

impl JitRuntime {
    /// Starts building an owning JIT runtime.
    pub fn builder() -> JitRuntimeBuilder {
        JitRuntimeBuilder::default()
    }

    /// Returns the disabled JIT guard.
    pub const fn jit(&self) -> &Jit {
        &self.jit
    }

    /// Returns runtime metrics.
    pub const fn metrics(&self) -> &JitMetrics {
        self.jit.metrics()
    }
}

impl Deref for JitRuntime {
    type Target = Runtime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
