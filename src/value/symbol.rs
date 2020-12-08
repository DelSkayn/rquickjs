use crate::Value;

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct Symbol<'js>(pub(crate) Value<'js>);
