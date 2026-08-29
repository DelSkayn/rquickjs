//! Optional tiered JIT integration for `rquickjs`.
//!
//! The runtime attaches through a versioned engine ABI while execution remains
//! on the QuickJS interpreter until compiler tiers are enabled.

pub mod abi;
mod config;
mod error;
mod metrics;

#[doc(hidden)]
#[path = "../tests/support/mod.rs"]
pub mod test_support;

use core::ops::Deref;

pub use config::{JitConfig, JitConfigBuilder, JitDiagnostic, JitDiagnosticKind};
pub use error::JitError;
pub use metrics::JitMetrics;
pub use rquickjs_core::Runtime;

const NATIVE_EXECUTION_SUPPORTED: bool = cfg!(any(
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
));

/// Owns the guard that keeps a JIT backend attached to a runtime.
#[derive(Debug)]
pub struct Jit {
    metrics: JitMetrics,
    _guard: rquickjs_core::runtime::RuntimeJitGuard,
}

impl Jit {
    /// Attaches the initial no-op backend to an existing runtime.
    pub fn attach(runtime: &Runtime, config: JitConfig) -> Result<Self, JitError> {
        let info = abi::AbiInfo::query_linked()?;
        Self::attach_with_info(runtime, config, info)
    }

    fn attach_with_info(
        runtime: &Runtime,
        config: JitConfig,
        info: abi::AbiInfo,
    ) -> Result<Self, JitError> {
        if let Err(error) = info.validate() {
            if let abi::AbiError::Incompatible(mismatch) = error {
                config.report(JitDiagnosticKind::AbiMismatch(mismatch));
            }
            return Err(error.into());
        }

        let metrics = JitMetrics::disabled();
        config.observe(&metrics);
        let guard = match runtime.attach_jit_backend(NoopBackend {
            _config: config.clone(),
        }) {
            Ok(guard) => guard,
            Err(error) => {
                config.report(JitDiagnosticKind::BackendAttachment);
                return Err(error.into());
            }
        };
        Ok(Self {
            metrics,
            _guard: guard,
        })
    }

    /// Fails when the current target cannot support native execution.
    pub fn require_native(&self) -> Result<(), JitError> {
        if NATIVE_EXECUTION_SUPPORTED {
            Ok(())
        } else {
            Err(JitError::UnsupportedPlatform)
        }
    }

    /// Returns the metrics associated with this backend guard.
    pub const fn metrics(&self) -> &JitMetrics {
        &self.metrics
    }
}

#[derive(Debug)]
struct NoopBackend {
    _config: JitConfig,
}

unsafe impl rquickjs_core::runtime::JitBackend for NoopBackend {}

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
