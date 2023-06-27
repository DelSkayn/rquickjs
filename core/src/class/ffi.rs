use std::mem;

use crate::{class::JsCell, qjs};

use super::JsClass;

pub(crate) unsafe extern "C" fn finalizer<'js, C: JsClass<'js>>(
    rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
) {
    let ptr = qjs::JS_GetOpaque(val, C::class_id().get()).cast::<JsCell<C>>();
    debug_assert!(!ptr.is_null());
    let inst = Box::from_raw(ptr);
    qjs::JS_FreeValueRT(rt, val);
    mem::drop(inst);
}
