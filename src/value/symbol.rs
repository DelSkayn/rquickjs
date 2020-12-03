use crate::{qjs, JsRef, JsRefType, Value};

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct Symbol<'js>(pub(crate) JsRef<'js, Self>);

impl<'js> JsRefType for Symbol<'js> {
    const TAG: i32 = qjs::JS_TAG_SYMBOL;
}

impl<'js> Symbol<'js> {
    /// Convert into value
    pub fn into_value(self) -> Value<'js> {
        Value::Symbol(self)
    }
}
