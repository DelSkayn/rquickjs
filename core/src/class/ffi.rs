use std::mem;

use crate::{class::JsCell, qjs};

use super::{JsClass, Mutability, Tracer};

pub(crate) unsafe extern "C" fn finalizer<'js, C: JsClass<'js>>(
    _rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
) {
    let ptr = qjs::JS_GetOpaque(val, C::class_id().get()).cast::<JsCell<C>>();
    debug_assert!(!ptr.is_null());
    let inst = Box::from_raw(ptr);
    mem::drop(inst);
}

pub(crate) unsafe extern "C" fn trace<'js, C: JsClass<'js>>(
    rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
    mark_func: qjs::JS_MarkFunc,
) {
    let id = C::class_id();
    let ptr = qjs::JS_GetOpaque(val, id.get()).cast::<JsCell<C>>();
    let tracer = Tracer::from_ffi(rt, mark_func);
    <C::Mutable as Mutability>::deref(&(*ptr).cell).trace(tracer)
}
