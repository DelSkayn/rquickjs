use crate::{JsSymbolRef, Value};

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol<'js>(pub(crate) JsSymbolRef<'js>);

impl<'js> Symbol<'js> {
    /// Convert into value
    pub fn into_value(self) -> Value<'js> {
        Value::Symbol(self)
    }
}
