use super::{JsClass, Mutability, Tracer};
use crate::{class::JsCell, qjs};
use std::mem;

/// FFI finalizer, destroying the object once it is delete by the Gc.
pub(crate) unsafe extern "C" fn finalizer<'js, C: JsClass<'js>>(
    _rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
) {
    let ptr = qjs::JS_GetOpaque(val, C::class_id().get()).cast::<JsCell<C>>();
    debug_assert!(!ptr.is_null());
    let inst = Box::from_raw(ptr);
    mem::drop(inst);
}

/// FFI tracing function.
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
