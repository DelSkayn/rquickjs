use crate::value::rf::JsSymbolRef;
//use rquickjs_sys as qjs;

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol<'js>(pub(crate) JsSymbolRef<'js>);

impl<'js> Symbol<'js> {}
