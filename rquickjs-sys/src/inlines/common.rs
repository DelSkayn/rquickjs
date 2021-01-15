pub use ::std::os::raw::{c_char, c_int, c_uint, c_void};

pub type JSValueConst = JSValue;

pub const JS_NULL: JSValue = JS_MKVAL(JS_TAG_NULL, 0);
pub const JS_UNDEFINED: JSValue = JS_MKVAL(JS_TAG_UNDEFINED, 0);
pub const JS_FALSE: JSValue = JS_MKVAL(JS_TAG_BOOL, 0);
pub const JS_TRUE: JSValue = JS_MKVAL(JS_TAG_BOOL, 1);
pub const JS_EXCEPTION: JSValue = JS_MKVAL(JS_TAG_EXCEPTION, 0);
pub const JS_UNINITIALIZED: JSValue = JS_MKVAL(JS_TAG_UNINITIALIZED, 0);

#[inline]
pub unsafe fn JS_VALUE_HAS_REF_COUNT(v: JSValue) -> bool {
    JS_VALUE_GET_TAG(v) as c_uint >= JS_TAG_FIRST as c_uint
}

#[inline]
pub unsafe fn JS_IsNumber(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_INT || JS_TAG_IS_FLOAT64(tag)
}

#[inline]
pub unsafe fn JS_IsInt(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_INT
}

#[inline]
pub unsafe fn JS_IsBigInt(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_BIG_INT
}

#[inline]
pub unsafe fn JS_IsBigFloat(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_BIG_FLOAT
}

#[inline]
pub unsafe fn JS_IsBigDecimal(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_BIG_DECIMAL
}

#[inline]
pub unsafe fn JS_IsBool(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_BOOL
}

#[inline]
pub unsafe fn JS_IsNull(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_NULL
}

#[inline]
pub unsafe fn JS_IsUndefined(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_UNDEFINED
}

#[inline]
pub unsafe fn JS_IsException(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_EXCEPTION
}

#[inline]
pub unsafe fn JS_IsUninitialized(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_UNINITIALIZED
}

#[inline]
pub unsafe fn JS_IsString(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_STRING
}

#[inline]
pub unsafe fn JS_IsSymbol(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_SYMBOL
}

#[inline]
pub unsafe fn JS_IsObject(v: JSValue) -> bool {
    let tag = JS_VALUE_GET_TAG(v);
    tag == JS_TAG_OBJECT
}

#[inline]
pub unsafe fn JS_ValueRefCount(v: JSValue) -> c_int {
    let p = &mut *(JS_VALUE_GET_PTR(v) as *mut JSRefCountHeader);
    p.ref_count
}

#[inline]
unsafe fn JS_FreeValueRef(ctx: *mut JSContext, v: JSValue) {
    let p = &mut *(JS_VALUE_GET_PTR(v) as *mut JSRefCountHeader);
    p.ref_count -= 1;
    if p.ref_count <= 0 {
        __JS_FreeValue(ctx, v)
    }
}

#[inline]
pub unsafe fn JS_FreeValue(ctx: *mut JSContext, v: JSValue) {
    if JS_VALUE_HAS_REF_COUNT(v) {
        JS_FreeValueRef(ctx, v);
    }
}

#[inline]
unsafe fn JS_FreeValueRefRT(rt: *mut JSRuntime, v: JSValue) {
    let p = &mut *(JS_VALUE_GET_PTR(v) as *mut JSRefCountHeader);
    p.ref_count -= 1;
    if p.ref_count <= 0 {
        __JS_FreeValueRT(rt, v)
    }
}

#[inline]
pub unsafe fn JS_FreeValueRT(rt: *mut JSRuntime, v: JSValue) {
    if JS_VALUE_HAS_REF_COUNT(v) {
        JS_FreeValueRefRT(rt, v);
    }
}

#[inline]
unsafe fn JS_DupValueRef(v: JSValueConst) -> JSValue {
    let p = &mut *(JS_VALUE_GET_PTR(v) as *mut JSRefCountHeader);
    p.ref_count += 1;
    v
}

#[inline]
pub unsafe fn JS_DupValue(v: JSValue) -> JSValue {
    if JS_VALUE_HAS_REF_COUNT(v) {
        JS_DupValueRef(v)
    } else {
        v
    }
}

#[inline]
pub unsafe fn JS_ToCString(ctx: *mut JSContext, val: JSValue) -> *const c_char {
    JS_ToCStringLen2(ctx, ptr::null_mut(), val, 0)
}
#[inline]
pub unsafe fn JS_ToCStringLen(
    ctx: *mut JSContext,
    plen: *mut size_t,
    val: JSValue,
) -> *const c_char {
    JS_ToCStringLen2(ctx, plen, val, 0)
}

#[inline]
pub unsafe fn JS_GetProperty(ctx: *mut JSContext, this_obj: JSValue, prop: JSAtom) -> JSValue {
    JS_GetPropertyInternal(ctx, this_obj, prop, this_obj, 0)
}

#[inline]
pub unsafe fn JS_SetProperty(
    ctx: *mut JSContext,
    this_obj: JSValue,
    prop: JSAtom,
    val: JSValue,
) -> i32 {
    JS_SetPropertyInternal(ctx, this_obj, prop, val, JS_PROP_THROW as i32)
}

#[inline]
pub fn JS_NewFloat64(d: f64) -> JSValue {
    union U {
        d: f64,
        u: u64,
    }

    let u = U { d };
    let val = d as i32;
    let t = U { d: val as f64 };
    /* -0 cannot be represented as integer, so we compare the bit
    representation */
    if unsafe { u.u == t.u } {
        JS_MKVAL(JS_TAG_INT, val)
    } else {
        __JS_NewFloat64(d)
    }
}
