//! JIT setup and configuration errors.

use core::fmt;

/// Errors returned while configuring or constructing a JIT runtime.
#[derive(Debug)]
pub enum JitError {
    /// A configuration value is outside its supported bounds.
    InvalidConfig(&'static str),
    /// Native execution is unavailable for this target.
    UnsupportedPlatform,
    /// Constructing the underlying QuickJS runtime failed.
    Runtime(rquickjs_core::Error),
    /// The linked engine exposes an incompatible JIT ABI.
    Abi(crate::abi::AbiError),
    /// QuickJS rejected backend registration.
    Backend(rquickjs_core::runtime::JitBackendAttachError),
}

impl From<rquickjs_core::Error> for JitError {
    fn from(error: rquickjs_core::Error) -> Self {
        Self::Runtime(error)
    }
}

impl From<crate::abi::AbiError> for JitError {
    fn from(error: crate::abi::AbiError) -> Self {
        Self::Abi(error)
    }
}

impl From<rquickjs_core::runtime::JitBackendAttachError> for JitError {
    fn from(error: rquickjs_core::runtime::JitBackendAttachError) -> Self {
        Self::Backend(error)
    }
}

impl fmt::Display for JitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(name) => write!(f, "invalid JIT configuration: {name}"),
            Self::UnsupportedPlatform => f.write_str("native JIT is unsupported on this platform"),
            Self::Runtime(error) => error.fmt(f),
            Self::Abi(error) => error.fmt(f),
            Self::Backend(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for JitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Abi(error) => Some(error),
            Self::InvalidConfig(_) | Self::UnsupportedPlatform | Self::Backend(_) => None,
        }
    }
}
