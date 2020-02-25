#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::unreadable_literal)]

use std::{mem, ptr};

mod bindings;
pub use bindings::*;

#[macro_export]
macro_rules! JS_VALUE_GET_TAG {
    ($v:expr) => {
        $v.tag as i32
    };
}

#[macro_export]
macro_rules! JS_VALUE_GET_NORM_TAG {
    ($v:expr) => {
        JS_VALUE_GET_TAG!($v)
    };
}

#[macro_export]
macro_rules! JS_VALUE_GET_INT {
    ($v:expr) => {
        $v.u.int32
    };
}

#[macro_export]
macro_rules! JS_VALUE_GET_BOOL {
    ($v:expr) => {
        $v.u.int32
    };
}

#[macro_export]
macro_rules! JS_VALUE_GET_FLOAT64 {
    ($v:expr) => {
        $v.u.float64
    };
}

#[macro_export]
macro_rules! JS_VALUE_GET_PTR {
    ($v:expr) => {
        $v.u.ptr
    };
}

#[macro_export]
macro_rules! JS_TAG_IS_FLOAT64 {
    ($tag:expr) => {
        $tag == JS_TAG_FLOAT64
    };
}

#[macro_export]
macro_rules! JS_NAN {
    () => {
        JSValue {
            u: JSValueUnion {
                float64: JS_FLOAT64_NAN,
            },
            tag: JS_TAG_FLOAT64 as i64,
        }
    };
}

#[macro_export]
macro_rules! JS_VALUE_HAS_REF_COUNT {
    ($tag:expr) => {
        $tag as libc::c_uint >= JS_TAG_FIRST as libc::c_uint
    };
}
#[inline]
pub fn __JSValue_NewFloat64(_ctx: *mut JSContext, d: f64) -> JSValue {
    JSValue {
        tag: JS_TAG_FLOAT64 as i64,
        u: JSValueUnion { float64: d },
    }
}

#[inline]
pub fn JS_IsNumber(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_INT || JS_TAG_IS_FLOAT64!(tag)
}

#[inline]
pub fn JS_IsInt(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_INT
}

#[inline]
pub fn JS_IsBigInt(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_BIG_INT
}

#[inline]
pub fn JS_IsBigFloat(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_BIG_FLOAT
}

#[inline]
pub fn JS_IsBigDecimal(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_BIG_DECIMAL
}

#[inline]
pub fn JS_IsBool(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_BOOL
}

#[inline]
pub fn JS_IsNull(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_NULL
}

#[inline]
pub fn JS_IsUndefined(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_UNDEFINED
}

#[inline]
pub fn JS_IsException(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_EXCEPTION
}

#[inline]
pub fn JS_IsUninitialized(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_UNINITIALIZED
}

#[inline]
pub fn JS_IsString(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_STRING
}

#[inline]
pub fn JS_IsSymbol(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_SYMBOL
}

#[inline]
pub fn JS_IsObject(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG!(v);
    tag == JS_TAG_OBJECT
}

#[inline]
pub unsafe fn JS_FreeValue(ctx: *mut JSContext, v: JSValue) {
    if JS_VALUE_HAS_REF_COUNT!(v.tag) {
        let p: *mut JSRefCountHeader = mem::transmute(JS_VALUE_GET_PTR!(v));
        (*p).ref_count -= 1;
        if (*p).ref_count <= 0 {
            __JS_FreeValue(ctx, v)
        }
    }
}

#[inline]
pub unsafe fn JS_FreeValueRT(rt: *mut JSRuntime, v: JSValue) {
    if JS_VALUE_HAS_REF_COUNT!(v.tag) {
        let p: *mut JSRefCountHeader = mem::transmute(JS_VALUE_GET_PTR!(v));
        (*p).ref_count -= 1;
        if (*p).ref_count <= 0 {
            __JS_FreeValueRT(rt, v)
        }
    }
}

#[inline]
pub unsafe fn JS_DupValue(v: JSValue) -> JSValue {
    if JS_VALUE_HAS_REF_COUNT!(v.tag) {
        let p: *mut JSRefCountHeader = mem::transmute(JS_VALUE_GET_PTR!(v));
        (*p).ref_count += 1;
    }
    v
}

#[inline]
pub unsafe fn JS_ToCString(ctx: *mut JSContext, val: JSValue) -> *const i8 {
    JS_ToCStringLen2(ctx, ptr::null_mut(), val, 0)
}

#[inline]
pub unsafe fn JS_GetProperty(ctx: *mut JSContext, this_obj: JSValue, prop: JSAtom) -> JSValue {
    JS_GetPropertyInternal(ctx, this_obj, prop, this_obj, 0)
}

#[inline]
pub unsafe fn JS_SetProperty(ctx: *mut JSContext, this_obj: JSValue, prop: JSAtom) -> i32 {
    JS_SetPropertyInternal(ctx, this_obj, prop, this_obj, JS_PROP_THROW as i32)
}
