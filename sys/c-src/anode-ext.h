#ifndef ANODE_EXT_H
#define ANODE_EXT_H
#include "quickjs-internals.h"

// This file is used to generate LLVM IR for the API of Anode.

#pragma region JSValueManipulations
// This section defines utility functions that acts the same as QuickJS's
// macros.

static inline int32_t anode_js_value_get_tag(JSValueConst val) {
  return JS_VALUE_GET_TAG(val);
}

static inline int32_t anode_js_value_get_norm_tag(JSValueConst val) {
  return JS_VALUE_GET_NORM_TAG(val);
}

static inline int32_t anode_js_value_get_int(JSValueConst val) {
  return JS_VALUE_GET_INT(val);
}

static inline int32_t anode_js_value_get_bool(JSValueConst val) {
  return JS_VALUE_GET_BOOL(val);
}

static inline int32_t anode_js_value_get_float64(JSValueConst val) {
  return JS_VALUE_GET_FLOAT64(val);
}

static inline void *anode_js_value_get_ptr(JSValueConst val) {
  return JS_VALUE_GET_PTR(val);
}

static inline JSValue anode_js_new_int32(int32_t tag, int32_t val) {
  return JS_MKVAL(tag, val);
}

static inline JSValue anode_js_new_ptr(int32_t tag, void *ptr) {
  return JS_MKPTR(tag, ptr);
}

static inline JSValue anode_js_new_float64(JSContext *ctx, double d) {
  return JS_NewFloat64(ctx, d);
}

#pragma endregion

JSValue anode_js_add_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_sub_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_mul_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_div_any(JSContext *ctx, JSValue x, JSValue y);

#endif
