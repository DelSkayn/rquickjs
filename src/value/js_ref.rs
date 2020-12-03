use crate::{qjs, Ctx};
use std::{fmt, marker::PhantomData, mem};

/// Trait to avoid code duplication over a single constant.
pub trait JsRefType {
    const TAG: i32;
}

/// A owned reference to a javascript object.
/// Handles the reference count of associated objects and
/// free's objects when nessacary. TODO spelling
pub struct JsRef<'js, Ty: JsRefType> {
    pub(crate) ctx: Ctx<'js>,
    pub(crate) value: qjs::JSValue,
    marker: PhantomData<Ty>,
}

impl<'js, Ty: JsRefType> fmt::Debug for JsRef<'js, Ty> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("JsRef")
            .field("ctx", &self.ctx)
            .field("ptr", &self.ptr())
            .finish()
    }
}

impl<'js, Ty: JsRefType> PartialEq for JsRef<'js, Ty> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr() == other.ptr()
    }
}

impl<'js, Ty: JsRefType> JsRef<'js, Ty> {
    /// creates a ref from a js value we have ownership of.
    pub unsafe fn from_js_value(ctx: Ctx<'js>, value: qjs::JSValue) -> Self {
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(value), Ty::TAG);
        JsRef {
            ctx,
            value,
            marker: PhantomData,
        }
    }

    /// creates a ref from a const js value.
    ///
    /// const js value represent a borrow of a js value which
    /// means that the ref_count is not increment for the current js value.
    /// so if we want to convert it to a JsRef we will first need to increment the ref count.
    pub unsafe fn from_js_value_const(ctx: Ctx<'js>, value: qjs::JSValueConst) -> Self {
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(value), Ty::TAG);
        JsRef {
            ctx,
            value: qjs::JS_DupValueRef(value),
            marker: PhantomData,
        }
    }

    /// return the underlying JSValue
    pub fn as_js_value(&self) -> qjs::JSValueConst {
        self.value
    }

    fn ptr(&self) -> *const qjs::c_void {
        unsafe { qjs::JS_VALUE_GET_PTR(self.value) }
    }

    /// return the underlying JSValue
    /// and consume the object, not decreasing the refcount
    /// on drop.
    pub fn into_js_value(self) -> qjs::JSValue {
        let value = self.value;
        mem::forget(self);
        value
    }
}

impl<'js, Ty: JsRefType> Clone for JsRef<'js, Ty> {
    fn clone(&self) -> Self {
        JsRef {
            ctx: self.ctx,
            value: unsafe { qjs::JS_DupValueRef(self.value) },
            marker: PhantomData,
        }
    }
}

impl<Ty: JsRefType> Drop for JsRef<'_, Ty> {
    fn drop(&mut self) {
        unsafe {
            qjs::JS_FreeValueRef(self.ctx.ctx, self.value);
        }
    }
}
