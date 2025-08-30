use super::{JsClass, Tracer};
use crate::{class::JsCell, function::Params, qjs, runtime::opaque::Opaque, Atom, Ctx, Value};
use alloc::boxed::Box;
use core::{any::TypeId, panic::AssertUnwindSafe, ptr::NonNull};

/// FFI finalizer, destroying the object once it is delete by the Gc.
pub(crate) unsafe extern "C" fn class_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let class_id = Opaque::from_runtime_ptr(rt).get_class_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.finalizer)(ptr);
}

/// FFI finalizer, destroying the object once it is delete by the Gc.
pub(crate) unsafe extern "C" fn exotic_class_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let class_id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.finalizer)(ptr);
}

/// FFI tracing function for non callable classes.
pub(crate) unsafe extern "C" fn class_trace(
    rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
    mark_func: qjs::JS_MarkFunc,
) {
    let class_id = Opaque::from_runtime_ptr(rt).get_class_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    let tracer = Tracer::from_ffi(rt, mark_func);
    (ptr.as_ref().v_table.trace)(ptr, tracer)
}

/// FFI tracing function for non callable exotic classes.
pub(crate) unsafe extern "C" fn exotic_class_trace(
    rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
    mark_func: qjs::JS_MarkFunc,
) {
    let class_id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    let tracer = Tracer::from_ffi(rt, mark_func);
    (ptr.as_ref().v_table.trace)(ptr, tracer)
}

/// FFI finalizer, destroying the object once it is delete by the Gc.
pub(crate) unsafe extern "C" fn callable_finalizer(rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
    let class_id = Opaque::from_runtime_ptr(rt).get_callable_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.finalizer)(ptr)
}

/// FFI tracing function for classes of type callable.
pub(crate) unsafe extern "C" fn callable_trace(
    rt: *mut qjs::JSRuntime,
    val: qjs::JSValue,
    mark_func: qjs::JS_MarkFunc,
) {
    let class_id = Opaque::from_runtime_ptr(rt).get_callable_id();
    let ptr = qjs::JS_GetOpaque(val, class_id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    let tracer = Tracer::from_ffi(rt, mark_func);
    (ptr.as_ref().v_table.trace)(ptr, tracer)
}

/// FFI calling function.
pub(crate) unsafe extern "C" fn call(
    ctx: *mut qjs::JSContext,
    function: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    flags: qjs::c_int,
) -> qjs::JSValue {
    let rt = qjs::JS_GetRuntime(ctx);
    let id = Opaque::from_runtime_ptr(rt).get_callable_id();
    let ptr = qjs::JS_GetOpaque(function, id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.call)(ptr, ctx, function, this, argc, argv, flags)
}

/// FFI exotic get_property function for classes with exotic behavior.
pub(crate) unsafe extern "C" fn exotic_get_property(
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
    receiver: qjs::JSValueConst,
) -> qjs::JSValue {
    let rt = qjs::JS_GetRuntime(ctx);
    let id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(obj, id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.get_property)(ptr, ctx, obj, atom, receiver)
}

/// FFI exotic set_property function for classes with exotic behavior.
pub(crate) unsafe extern "C" fn exotic_set_property(
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
    value: qjs::JSValue,
    receiver: qjs::JSValueConst,
    flags: qjs::c_int,
) -> qjs::c_int {
    let rt = qjs::JS_GetRuntime(ctx);
    let id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(obj, id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.set_property)(ptr, ctx, obj, atom, receiver, value, flags)
}

/// FFI exotic has_property function for classes with exotic behavior.
pub(crate) unsafe extern "C" fn exotic_has_property(
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
) -> qjs::c_int {
    let rt = qjs::JS_GetRuntime(ctx);
    let id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(obj, id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.has_property)(ptr, ctx, obj, atom)
}

/// FFI exotic delete_property function for classes with exotic behavior.
pub(crate) unsafe extern "C" fn exotic_delete_property(
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    prop: qjs::JSAtom,
) -> qjs::c_int {
    let rt = qjs::JS_GetRuntime(ctx);
    let id = Opaque::from_runtime_ptr(rt).get_exotic_id();
    let ptr = qjs::JS_GetOpaque(obj, id);
    let ptr = NonNull::new(ptr).unwrap().cast::<ClassCell<()>>();
    (ptr.as_ref().v_table.delete_property)(ptr, ctx, obj, prop)
}

pub(crate) type FinalizerFunc = unsafe fn(this: NonNull<ClassCell<()>>);
pub(crate) type TraceFunc =
    for<'a> unsafe fn(this: NonNull<ClassCell<()>>, tracer: Tracer<'a, 'static>);
pub(crate) type CallFunc = for<'a> unsafe fn(
    this_ptr: NonNull<ClassCell<()>>,
    ctx: *mut qjs::JSContext,
    function: qjs::JSValue,
    this: qjs::JSValue,
    argc: qjs::c_int,
    argv: *mut qjs::JSValue,
    flags: qjs::c_int,
) -> qjs::JSValue;

pub(crate) type GetPropertyFunc = for<'a> unsafe fn(
    this_ptr: NonNull<ClassCell<()>>,
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
    receiver: qjs::JSValueConst,
) -> qjs::JSValue;

pub(crate) type SetPropertyFunc = for<'a> unsafe fn(
    this_ptr: NonNull<ClassCell<()>>,
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
    receiver: qjs::JSValueConst,
    value: qjs::JSValue,
    flags: qjs::c_int,
) -> qjs::c_int;

pub(crate) type HasPropertyFunc = for<'a> unsafe fn(
    this_ptr: NonNull<ClassCell<()>>,
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    atom: qjs::JSAtom,
) -> qjs::c_int;

pub(crate) type DeletePropertyFunc = for<'a> unsafe fn(
    this_ptr: NonNull<ClassCell<()>>,
    ctx: *mut qjs::JSContext,
    obj: qjs::JSValueConst,
    prop: qjs::JSAtom,
) -> qjs::c_int;

pub(crate) type TypeIdFn = fn() -> TypeId;

pub(crate) struct VTable {
    id_fn: TypeIdFn,
    finalizer: FinalizerFunc,
    trace: TraceFunc,
    call: CallFunc,
    get_property: GetPropertyFunc,
    set_property: SetPropertyFunc,
    has_property: HasPropertyFunc,
    delete_property: DeletePropertyFunc,
}

impl VTable {
    unsafe fn finalizer_impl<'js, C: JsClass<'js>>(this: NonNull<ClassCell<()>>) {
        let this = this.cast::<ClassCell<JsCell<C>>>();
        let _ = Box::from_raw(this.as_ptr());
    }

    unsafe fn trace_impl<'js, C: JsClass<'js>>(
        this: NonNull<ClassCell<()>>,
        tracer: Tracer<'_, 'static>,
    ) {
        let this = this.cast::<ClassCell<JsCell<C>>>();
        if let Ok(x) = this.as_ref().data.try_borrow() {
            x.trace(tracer.cast_js_lifetime())
        }
    }

    unsafe fn call_impl<'js, C: JsClass<'js>>(
        this_ptr: NonNull<ClassCell<()>>,
        ctx: *mut qjs::JSContext,
        function: qjs::JSValue,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
        flags: qjs::c_int,
    ) -> qjs::JSValue {
        let this_ptr = this_ptr.cast::<ClassCell<JsCell<C>>>();
        let params = Params::from_ffi_class(ctx, function, this, argc, argv, flags);
        let ctx = params.ctx().clone();

        ctx.handle_panic(AssertUnwindSafe(|| {
            C::call(&this_ptr.as_ref().data, params)
                .map(Value::into_js_value)
                .unwrap_or_else(|e| e.throw(&ctx))
        }))
    }

    unsafe fn get_property_impl<'js, C: JsClass<'js>>(
        this_ptr: NonNull<ClassCell<()>>,
        ctx: *mut qjs::JSContext,
        obj: qjs::JSValueConst,
        atom: qjs::JSAtom,
        receiver: qjs::JSValueConst,
    ) -> qjs::JSValue {
        let this_ptr = this_ptr.cast::<ClassCell<JsCell<C>>>();
        let ctx = Ctx::from_ptr(ctx);
        let atom = Atom::from_atom_val_dup(ctx.clone(), atom);
        let obj = Value::from_js_value_const(ctx.clone(), obj);
        let receiver = Value::from_js_value_const(ctx.clone(), receiver);

        ctx.handle_panic(AssertUnwindSafe(|| {
            C::exotic_get_property(&this_ptr.as_ref().data, &ctx, atom, obj, receiver)
                .map(Value::into_js_value)
                .unwrap_or_else(|e| e.throw(&ctx))
        }))
    }

    unsafe fn set_property_impl<'js, C: JsClass<'js>>(
        this_ptr: NonNull<ClassCell<()>>,
        ctx: *mut qjs::JSContext,
        obj: qjs::JSValueConst,
        atom: qjs::JSAtom,
        receiver: qjs::JSValueConst,
        value: qjs::JSValue,
        _flags: qjs::c_int,
    ) -> qjs::c_int {
        let this_ptr = this_ptr.cast::<ClassCell<JsCell<C>>>();
        let ctx = Ctx::from_ptr(ctx);
        let atom = Atom::from_atom_val_dup(ctx.clone(), atom);
        let obj = Value::from_js_value_const(ctx.clone(), obj);
        let receiver = Value::from_js_value_const(ctx.clone(), receiver);
        let value = Value::from_js_value(ctx.clone(), value);

        ctx.handle_panic_exotic(AssertUnwindSafe(|| {
            match C::exotic_set_property(&this_ptr.as_ref().data, &ctx, atom, obj, receiver, value)
            {
                Ok(v) => {
                    if v {
                        1
                    } else {
                        0
                    }
                }
                Err(e) => {
                    e.throw(&ctx);
                    -1
                }
            }
        }))
    }

    unsafe fn has_property_impl<'js, C: JsClass<'js>>(
        this_ptr: NonNull<ClassCell<()>>,
        ctx: *mut qjs::JSContext,
        obj: qjs::JSValueConst,
        atom: qjs::JSAtom,
    ) -> qjs::c_int {
        let this_ptr = this_ptr.cast::<ClassCell<JsCell<C>>>();
        let ctx = Ctx::from_ptr(ctx);
        let atom = Atom::from_atom_val_dup(ctx.clone(), atom);
        let obj = Value::from_js_value_const(ctx.clone(), obj);

        ctx.handle_panic_exotic(AssertUnwindSafe(|| {
            match C::exotic_has_property(&this_ptr.as_ref().data, &ctx, atom, obj) {
                Ok(v) => {
                    if v {
                        1
                    } else {
                        0
                    }
                }
                Err(e) => {
                    e.throw(&ctx);
                    -1
                }
            }
        }))
    }

    unsafe fn delete_property_impl<'js, C: JsClass<'js>>(
        this_ptr: NonNull<ClassCell<()>>,
        ctx: *mut qjs::JSContext,
        obj: qjs::JSValueConst,
        atom: qjs::JSAtom,
    ) -> qjs::c_int {
        let this_ptr = this_ptr.cast::<ClassCell<JsCell<C>>>();
        let ctx = Ctx::from_ptr(ctx);
        let atom = Atom::from_atom_val_dup(ctx.clone(), atom);
        let obj = Value::from_js_value_const(ctx.clone(), obj);

        ctx.handle_panic_exotic(AssertUnwindSafe(|| {
            match C::exotic_delete_property(&this_ptr.as_ref().data, &ctx, atom, obj) {
                Ok(v) => {
                    if v {
                        1
                    } else {
                        0
                    }
                }
                Err(e) => {
                    e.throw(&ctx);
                    -1
                }
            }
        }))
    }

    pub fn get<'js, C: JsClass<'js>>() -> &'static VTable {
        trait HasVTable {
            const VTABLE: VTable;
        }

        impl<'js, C: JsClass<'js>> HasVTable for C {
            const VTABLE: VTable = VTable {
                id_fn: TypeId::of::<C::Changed<'static>>,
                finalizer: VTable::finalizer_impl::<'js, C>,
                trace: VTable::trace_impl::<C>,
                call: VTable::call_impl::<C>,
                get_property: VTable::get_property_impl::<C>,
                set_property: VTable::set_property_impl::<C>,
                has_property: VTable::has_property_impl::<C>,
                delete_property: VTable::delete_property_impl::<C>,
            };
        }
        &<C as HasVTable>::VTABLE
    }

    pub fn id(&self) -> TypeId {
        (self.id_fn)()
    }

    pub fn is_of_class<'js, C: JsClass<'js>>(&self) -> bool {
        (self.id_fn)() == TypeId::of::<C::Changed<'static>>()
    }
}

#[repr(C)]
pub(crate) struct ClassCell<T> {
    pub(crate) v_table: &'static VTable,
    pub(crate) data: T,
}

impl<'js, T: JsClass<'js>> ClassCell<JsCell<'js, T>> {
    pub(crate) fn new(class: T) -> Self {
        ClassCell {
            v_table: VTable::get::<T>(),
            data: JsCell::new(class),
        }
    }
}
