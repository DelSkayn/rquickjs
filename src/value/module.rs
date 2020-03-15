use crate::{value::rf::JsModuleRef, Ctx};
use rquickjs_sys as qjs;

/// Javascript module with certain exports and imports
#[derive(Debug, PartialEq)]
pub struct Module<'js>(JsModuleRef<'js>);

impl<'js> Module<'js> {
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, js_val: qjs::JSValue) -> Self {
        Module(JsModuleRef::from_js_value(ctx, js_val))
    }

    #[allow(dead_code)]
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }
}
