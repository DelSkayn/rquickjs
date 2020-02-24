use crate::context::Ctx;
use rquickjs_sys as qjs;
use std::marker::PhantomData;

/// A owned reference to a javascript object
#[derive(Debug)]
pub struct JsRef<'js, Ty: JsRefType> {
    pub(crate) ctx: Ctx<'js>,
    pub(crate) ptr: *mut libc::c_void,
    marker: PhantomData<Ty>,
}

impl<'js, Ty: JsRefType> PartialEq for JsRef<'js, Ty> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<'js, Ty: JsRefType> JsRef<'js, Ty> {
    pub unsafe fn from_js_value(ctx: Ctx<'js>, val: qjs::JSValue) -> Self {
        debug_assert_eq!(val.tag, Ty::TAG);
        JsRef {
            ctx,
            ptr: val.u.ptr,
            marker: PhantomData,
        }
    }

    pub fn as_js_value(&self) -> qjs::JSValue {
        qjs::JSValue {
            u: qjs::JSValueUnion { ptr: self.ptr },
            tag: Ty::TAG,
        }
    }
}

impl<'js, Ty: JsRefType> Clone for JsRef<'js, Ty> {
    fn clone(&self) -> Self {
        unsafe {
            let p = self.ptr as *mut qjs::JSRefCountHeader;
            (*p).ref_count += 1;
        }
        JsRef {
            ctx: self.ctx,
            ptr: self.ptr,
            marker: PhantomData,
        }
    }
}

impl<Ty: JsRefType> Drop for JsRef<'_, Ty> {
    fn drop(&mut self) {
        unsafe {
            let p = self.ptr as *mut qjs::JSRefCountHeader;
            (*p).ref_count -= 1;
            if (*p).ref_count <= 0 {
                let v = self.as_js_value();
                qjs::__JS_FreeValue(self.ctx.ctx, v);
            }
        }
    }
}

pub trait JsRefType {
    const TAG: i64;
}

pub struct JsStringType;

impl JsRefType for JsStringType {
    const TAG: i64 = qjs::JS_TAG_STRING as i64;
}

pub struct JsObjectType;

impl JsRefType for JsObjectType {
    const TAG: i64 = qjs::JS_TAG_OBJECT as i64;
}

pub struct JsModuleType;

impl JsRefType for JsModuleType {
    const TAG: i64 = qjs::JS_TAG_MODULE as i64;
}

pub struct JsSymbolType;

impl JsRefType for JsSymbolType {
    const TAG: i64 = qjs::JS_TAG_SYMBOL as i64;
}

pub type JsStringRef<'js> = JsRef<'js, JsStringType>;
pub type JsObjectRef<'js> = JsRef<'js, JsObjectType>;
pub type JsModuleRef<'js> = JsRef<'js, JsModuleType>;
pub type JsSymbolRef<'js> = JsRef<'js, JsSymbolType>;
