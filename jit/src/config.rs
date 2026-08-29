//! JIT policy and resource limits.

use std::sync::Arc;

use crate::{abi::AbiMismatch, JitError, JitMetrics};

pub const DEFAULT_CALL_THRESHOLD: u32 = 32;
pub const DEFAULT_LOOP_THRESHOLD: u32 = 56;
pub const DEFAULT_MAX_CODE_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_MAX_QUEUE_LEN: usize = 256;
pub const DEFAULT_WORKERS: usize = 1;
pub const DEFAULT_MAX_COMPILE_ATTEMPTS: u8 = 4;

/// Bounded policy and resource limits for one JIT runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JitDiagnosticKind {
    AbiMismatch(AbiMismatch),
    BackendAttachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JitDiagnostic {
    kind: JitDiagnosticKind,
}

impl JitDiagnostic {
    pub const fn kind(&self) -> &JitDiagnosticKind {
        &self.kind
    }
}

type DiagnosticCallback = Arc<dyn Fn(&JitDiagnostic) + Send + Sync>;
type MetricsObserver = Arc<dyn Fn(&JitMetrics) + Send + Sync>;

#[derive(Clone)]
pub struct JitConfig {
    call_threshold: u32,
    loop_threshold: u32,
    max_code_bytes: usize,
    max_queue_len: usize,
    workers: usize,
    max_compile_attempts: u8,
    diagnostic_callback: Option<DiagnosticCallback>,
    metrics_observer: Option<MetricsObserver>,
}

impl core::fmt::Debug for JitConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JitConfig")
            .field("call_threshold", &self.call_threshold)
            .field("loop_threshold", &self.loop_threshold)
            .field("max_code_bytes", &self.max_code_bytes)
            .field("max_queue_len", &self.max_queue_len)
            .field("workers", &self.workers)
            .field("max_compile_attempts", &self.max_compile_attempts)
            .field(
                "has_diagnostic_callback",
                &self.diagnostic_callback.is_some(),
            )
            .field("has_metrics_observer", &self.metrics_observer.is_some())
            .finish()
    }
}

impl PartialEq for JitConfig {
    fn eq(&self, other: &Self) -> bool {
        self.call_threshold == other.call_threshold
            && self.loop_threshold == other.loop_threshold
            && self.max_code_bytes == other.max_code_bytes
            && self.max_queue_len == other.max_queue_len
            && self.workers == other.workers
            && self.max_compile_attempts == other.max_compile_attempts
            && callbacks_equal(&self.diagnostic_callback, &other.diagnostic_callback)
            && callbacks_equal(&self.metrics_observer, &other.metrics_observer)
    }
}

impl Eq for JitConfig {}

fn callbacks_equal<T: ?Sized>(left: &Option<Arc<T>>, right: &Option<Arc<T>>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

impl JitConfig {
    /// Starts configuring a JIT runtime with bounded defaults.
    pub fn builder() -> JitConfigBuilder {
        JitConfigBuilder::default()
    }

    pub const fn call_threshold(&self) -> u32 {
        self.call_threshold
    }

    pub const fn loop_threshold(&self) -> u32 {
        self.loop_threshold
    }

    pub const fn max_code_bytes(&self) -> usize {
        self.max_code_bytes
    }

    pub const fn max_queue_len(&self) -> usize {
        self.max_queue_len
    }

    pub const fn workers(&self) -> usize {
        self.workers
    }

    pub const fn max_compile_attempts(&self) -> u8 {
        self.max_compile_attempts
    }

    pub(crate) fn report(&self, kind: JitDiagnosticKind) {
        if let Some(callback) = &self.diagnostic_callback {
            callback(&JitDiagnostic { kind });
        }
    }

    pub(crate) fn observe(&self, metrics: &JitMetrics) {
        if let Some(callback) = &self.metrics_observer {
            callback(metrics);
        }
    }
}

impl Default for JitConfig {
    fn default() -> Self {
        Self {
            call_threshold: DEFAULT_CALL_THRESHOLD,
            loop_threshold: DEFAULT_LOOP_THRESHOLD,
            max_code_bytes: DEFAULT_MAX_CODE_BYTES,
            max_queue_len: DEFAULT_MAX_QUEUE_LEN,
            workers: DEFAULT_WORKERS,
            max_compile_attempts: DEFAULT_MAX_COMPILE_ATTEMPTS,
            diagnostic_callback: None,
            metrics_observer: None,
        }
    }
}

/// Builder for [`JitConfig`].
#[derive(Clone, Debug, Default)]
pub struct JitConfigBuilder {
    config: JitConfig,
}

impl JitConfigBuilder {
    pub fn call_threshold(mut self, value: u32) -> Self {
        self.config.call_threshold = value;
        self
    }

    pub fn loop_threshold(mut self, value: u32) -> Self {
        self.config.loop_threshold = value;
        self
    }

    pub fn max_code_bytes(mut self, value: usize) -> Self {
        self.config.max_code_bytes = value;
        self
    }

    pub fn max_queue_len(mut self, value: usize) -> Self {
        self.config.max_queue_len = value;
        self
    }

    pub fn workers(mut self, value: usize) -> Self {
        self.config.workers = value;
        self
    }

    pub fn max_compile_attempts(mut self, value: u8) -> Self {
        self.config.max_compile_attempts = value;
        self
    }

    pub fn diagnostic_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&JitDiagnostic) + Send + Sync + 'static,
    {
        self.config.diagnostic_callback = Some(Arc::new(callback));
        self
    }

    pub fn metrics_observer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&JitMetrics) + Send + Sync + 'static,
    {
        self.config.metrics_observer = Some(Arc::new(callback));
        self
    }

    pub fn build(self) -> Result<JitConfig, JitError> {
        if self.config.call_threshold == 0 {
            return Err(JitError::InvalidConfig("call_threshold"));
        }
        if self.config.loop_threshold == 0 {
            return Err(JitError::InvalidConfig("loop_threshold"));
        }
        if self.config.max_code_bytes == 0 {
            return Err(JitError::InvalidConfig("max_code_bytes"));
        }
        if self.config.max_queue_len == 0 {
            return Err(JitError::InvalidConfig("max_queue_len"));
        }
        if self.config.workers == 0 {
            return Err(JitError::InvalidConfig("workers"));
        }
        if self.config.max_compile_attempts == 0 {
            return Err(JitError::InvalidConfig("max_compile_attempts"));
        }
        Ok(self.config)
    }
}
