use crate::{value::rf::JsSymbolRef, Ctx};
use rquickjs_sys as qjs;

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol<'js>(JsSymbolRef<'js>);

impl<'js> Symbol<'js> {
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, val: qjs::JSValue) -> Self {
        Symbol(JsSymbolRef::from_js_value(ctx, val))
    }

    // Save because using JSValue in any way is unsafe.
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }
}
