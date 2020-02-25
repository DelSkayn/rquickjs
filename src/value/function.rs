use crate::{value::rf::JsObjectRef, Ctx, Object};
use rquickjs_sys as qjs;

#[derive(Debug, Clone, PartialEq)]
pub struct Function<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Function<'js> {
    // Unsafe because of requirement that the JSValue is valid.
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, val: qjs::JSValue) -> Self {
        Function(JsObjectRef::from_js_value(ctx, val))
    }

    // Safe because using JSValue is unsafe
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }

    pub fn to_object(self) -> Object<'js> {
        Object(self.0)
    }
}
