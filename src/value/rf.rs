use crate::{context::Ctx, qjs};
use std::{fmt, marker::PhantomData, mem};

/// A owned reference to a javascript object.
/// Handles the reference count of associated objects and
/// free's objects when nessacary. TODO spelling
pub struct JsRef<'js, Ty: JsRefType> {
    pub(crate) ctx: Ctx<'js>,
    pub(crate) ptr: *mut qjs::c_void,
    marker: PhantomData<Ty>,
}

impl<'js, Ty: JsRefType> fmt::Debug for JsRef<'js, Ty> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("JsRef")
            .field("ctx", &self.ctx)
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl<'js, Ty: JsRefType> PartialEq for JsRef<'js, Ty> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<'js, Ty: JsRefType> JsRef<'js, Ty> {
    /// creates a ref from a js value we have ownership of.
    pub unsafe fn from_js_value(ctx: Ctx<'js>, val: qjs::JSValue) -> Self {
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(val), Ty::TAG);
        JsRef {
            ctx,
            ptr: qjs::JS_VALUE_GET_PTR(val),
            marker: PhantomData,
        }
    }

    /// creates a ref from a const js value.
    ///
    /// const js value represent a borrow of a js value which
    /// means that the ref_count is not increment for the current js value.
    /// so if we want to convert it to a JsRef we will first need to increment the ref count.
    pub unsafe fn from_js_value_const(ctx: Ctx<'js>, val: qjs::JSValue) -> Self {
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(val), Ty::TAG);
        let ptr = qjs::JS_VALUE_GET_PTR(val);
        let p = &mut *(ptr as *mut qjs::JSRefCountHeader);
        p.ref_count += 1;
        JsRef {
            ctx,
            ptr,
            marker: PhantomData,
        }
    }

    /// return the underlying JSValue
    pub fn as_js_value(&self) -> qjs::JSValue {
        qjs::JS_MKPTR(Ty::TAG, self.ptr)
    }

    /// return the underlying JSValue
    /// and consume the object, not decreasing the refcount
    /// on drop.
    pub fn into_js_value(self) -> qjs::JSValue {
        let val = self.as_js_value();
        mem::forget(self);
        val
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

/// Trait to avoid code duplication over a single constant.
pub trait JsRefType {
    const TAG: i32;
}

pub struct JsStringType;

impl JsRefType for JsStringType {
    const TAG: i32 = qjs::JS_TAG_STRING;
}

pub struct JsObjectType;

impl JsRefType for JsObjectType {
    const TAG: i32 = qjs::JS_TAG_OBJECT;
}

pub struct JsSymbolType;

impl JsRefType for JsSymbolType {
    const TAG: i32 = qjs::JS_TAG_SYMBOL;
}

pub type JsStringRef<'js> = JsRef<'js, JsStringType>;
pub type JsObjectRef<'js> = JsRef<'js, JsObjectType>;
pub type JsSymbolRef<'js> = JsRef<'js, JsSymbolType>;
