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
}

impl From<rquickjs_core::Error> for JitError {
    fn from(error: rquickjs_core::Error) -> Self {
        Self::Runtime(error)
    }
}

impl fmt::Display for JitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(name) => write!(f, "invalid JIT configuration: {name}"),
            Self::UnsupportedPlatform => f.write_str("native JIT is unsupported on this platform"),
            Self::Runtime(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for JitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::InvalidConfig(_) | Self::UnsupportedPlatform => None,
        }
    }
}
