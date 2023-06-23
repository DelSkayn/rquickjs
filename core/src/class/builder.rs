pub struct ClassBuilder<'js> {
    ctx: Ctx<'js>,
    prototype: Option<Object<'js>>,
    func: Option<
        unsafe extern "C" fn(
            *mut qjs::JSContext,
            JSValue,
            JSValue,
            i32,
            *mut JSValue,
            i32,
        ) -> JSValue,
    >,
}
