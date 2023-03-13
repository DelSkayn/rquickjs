use super::Input;
use crate::{handle_panic, qjs, ClassId, Ctx, Result, Value};
use std::{convert::TryInto, ops::Deref, panic::AssertUnwindSafe, ptr};

static FUNC_CLASS_ID: ClassId = ClassId::new();

type BoxedFunc<'js> = Box<dyn Fn(&Input<'js>) -> Result<Value<'js>>>;

pub struct JsFunction<'js> {
    length: usize,
    func: BoxedFunc<'js>,
}

impl<'js> Deref for JsFunction<'js> {
    type Target = BoxedFunc<'js>;

    fn deref(&self) -> &Self::Target {
        &self.func
    }
}

impl<'js> JsFunction<'js> {
    pub fn new<F>(length: usize, func: F) -> Self
    where
        F: Fn(&Input<'js>) -> Result<Value<'js>> + 'static,
    {
        Self {
            length,
            func: Box::new(func),
        }
    }

    pub fn class_id() -> qjs::JSClassID {
        FUNC_CLASS_ID.get() as _
    }

    pub unsafe fn into_js_value(self, ctx: Ctx<'_>) -> qjs::JSValue {
        let length = self
            .length
            .try_into()
            .expect("function argument length exceeded i32::MAX");
        let ptr = Box::into_raw(Box::new(self.func));
        let finalizer = qjs::JS_NewObjectClass(ctx.ctx, Self::class_id() as _);
        qjs::JS_SetOpaque(finalizer, ptr as _);

        let mut data = finalizer;
        let function = qjs::JS_NewCFunctionData(
            ctx.ctx,
            Some(Self::call),
            length,
            0,
            1,
            (&mut data) as *mut _,
        );
        qjs::JS_FreeValue(ctx.ctx, finalizer);
        function
    }

    unsafe fn _call(
        &self,
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> Result<qjs::JSValue> {
        let input = Input::new_raw(ctx, this, argc, argv);

        let res = (self.func)(&input)?;

        Ok(res.into_js_value())
    }

    pub unsafe fn register(rt: *mut qjs::JSRuntime) {
        let class_id = Self::class_id();
        if 0 == qjs::JS_IsRegisteredClass(rt, class_id) {
            let class_def = qjs::JSClassDef {
                class_name: b"RustFunctionFinalizer\0".as_ptr() as *const _,
                finalizer: Some(Self::finalizer),
                gc_mark: None,
                call: None,
                exotic: ptr::null_mut(),
            };
            assert!(qjs::JS_NewClass(rt, class_id, &class_def) == 0);
        }
    }

    unsafe extern "C" fn call(
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
        _magic: qjs::c_int,
        data: *mut qjs::JSValue,
    ) -> qjs::JSValue {
        let ctx = Ctx::from_ptr(ctx);
        let opaque =
            &*(qjs::JS_GetOpaque2(ctx.ctx, *data, Self::class_id()).cast::<BoxedFunc<'js>>());

        handle_panic(
            ctx.ctx,
            AssertUnwindSafe(|| {
                let input = Input::new_raw(ctx.ctx, this, argc, argv);
                (opaque)(&input)
                    .map(|x| x.into_js_value())
                    .unwrap_or_else(|error| error.throw(ctx))
            }),
        )
    }

    unsafe extern "C" fn finalizer(_rt: *mut qjs::JSRuntime, val: qjs::JSValue) {
        let _opaque =
            Box::<BoxedFunc<'js>>::from_raw(qjs::JS_GetOpaque(val, Self::class_id()).cast());
    }
}
