pub const JS_NAN_BOXING_OFFSET: u64 = 0xfff8 << 48;

#[inline]
pub unsafe fn JS_VALUE_GET_TAG(v: JSValue) -> i32 {
    (v >> 47 & 0xF) as i32
}

#[inline]
pub unsafe fn JS_VALUE_GET_INT(v: JSValue) -> i32 {
    (v & 0xFFFFFFFF) as i32
}

#[inline]
pub unsafe fn JS_VALUE_GET_BOOL(v: JSValue) -> bool {
    JS_VALUE_GET_INT(v) != 0
}

#[inline]
pub unsafe fn JS_VALUE_GET_PTR(v: JSValue) -> *mut c_void {
    (((v << 17) as i64) >> 16) as *mut c_void
}

#[inline]
pub const fn JS_MKVAL(tag: i32, val: i32) -> JSValue {
    ((tag as u64) << 47) | val as u64
}

#[inline]
pub fn JS_MKPTR(tag: i32, ptr: *mut c_void) -> JSValue {
    ((tag as u64) << 47) | ((ptr as usize as u64 >> 1) & ((1 << 47) - 1))
}

pub const JS_NAN: JSValue = JS_MKVAL(JS_TAG_FLOAT64, 0);

#[inline]
pub unsafe fn JS_VALUE_GET_FLOAT64(v: JSValue) -> f64 {
    let vv = v.wrapping_add(JS_NAN_BOXING_OFFSET);
    f64::from_bits(vv)
}

#[inline]
pub fn __JS_NewFloat64(d: f64) -> JSValue {
    let u = d.to_bits();
    if (u & 0x7fffffffffffffff) > 0x7ff0000000000000 {
        JS_NAN
    } else {
        u.wrapping_sub(JS_NAN_BOXING_OFFSET)
    }
}

#[inline]
pub fn JS_TAG_IS_FLOAT64(tag: i32) -> bool {
    ((tag.wrapping_sub(JS_TAG_FIRST)) as u32) >= (JS_TAG_FLOAT64 - JS_TAG_FIRST) as u32
}

#[inline]
pub unsafe fn JS_VALUE_GET_NORM_TAG(v: JSValue) -> i32 {
    let tag = JS_VALUE_GET_TAG(v);
    if JS_TAG_IS_FLOAT64(tag) {
        JS_TAG_FLOAT64
    } else {
        tag
    }
}

#[inline]
pub fn JS_VALUE_IS_NAN(v: JSValue) -> bool {
    v == JS_NAN
}
