//! JIT policy and resource limits.

use crate::JitError;

pub const DEFAULT_CALL_THRESHOLD: u32 = 32;
pub const DEFAULT_LOOP_THRESHOLD: u32 = 56;
pub const DEFAULT_MAX_CODE_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_MAX_QUEUE_LEN: usize = 256;
pub const DEFAULT_WORKERS: usize = 1;
pub const DEFAULT_MAX_COMPILE_ATTEMPTS: u8 = 4;

/// Bounded policy and resource limits for one JIT runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JitConfig {
    call_threshold: u32,
    loop_threshold: u32,
    max_code_bytes: usize,
    max_queue_len: usize,
    workers: usize,
    max_compile_attempts: u8,
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
        }
    }
}

/// Builder for [`JitConfig`].
#[derive(Clone, Debug)]
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

impl Default for JitConfigBuilder {
    fn default() -> Self {
        Self {
            config: JitConfig::default(),
        }
    }
}
