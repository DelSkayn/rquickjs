pub struct DropHook;

static mut DROP_HOOK_CLASS: qjs::JSClassID = 0;

impl DropHook {
    pub unsafe fn register(rt: *mut qjs::JSRuntime) {
        qjs::JS_NewClassID(&mut DROP_HOOK_CLASS);
        if 0 == qjs::JS_IsRegisteredClass(rt, FUNC_CLASS) {
            let class_def = qjs::JSClassDef {
                class_name: b"RustRuntimeDropHook\0".as_ptr() as *const _,
                finalizer: Some(Self::finalizer),
                gc_mark: None,
                call: Some(Self::call),
                exotic: ptr::null_mut(),
            };
            assert!(qjs::JS_NewClass(rt, FUNC_CLASS, &class_def) == 0);
        }
    }

    unsafe extern "C" fn finalizer(_rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let _opaque = Box::from_raw(qjs::JS_GetOpaque(val, DROP_HOOK_CLASS) as *mut Self);
    }
}
