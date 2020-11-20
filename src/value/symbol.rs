use crate::value::rf::JsSymbolRef;

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol<'js>(pub(crate) JsSymbolRef<'js>);

impl<'js> Symbol<'js> {}
