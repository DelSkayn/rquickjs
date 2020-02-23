use crate::{value::rf::JsModuleRef, Ctx};
use rquickjs_sys as qjs;

/// A module with certain exports and imports
#[derive(Debug, PartialEq)]
pub struct Module<'js>(JsModuleRef<'js>);

impl<'js> Module<'js> {
    pub(crate) unsafe fn new(ctx: Ctx<'js>, js_val: qjs::JSValue) -> Self {
        Module(JsModuleRef::from_js_value(ctx, js_val))
    }
}
