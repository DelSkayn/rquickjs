#include "anode-ext.h"
#include "quickjs.h"
#include <stdint.h>

JSValue anode_js_add_any(JSContext *ctx, JSValue x, JSValue y) {
  if (JS_VALUE_IS_BOTH_INT(x, y)) {
    int32_t sum = JS_VALUE_GET_INT(x) + JS_VALUE_GET_INT(y);
    // check for overflow
    if ((sum ^ JS_VALUE_GET_INT(x)) >= 0) {
      return JS_NewInt32(ctx, sum);
    } else {
      return JS_NewFloat64(ctx, (double)JS_VALUE_GET_INT(x) +
                                    (double)JS_VALUE_GET_INT(y));
    }
  } else if (JS_VALUE_IS_BOTH_FLOAT(x, y)) {
    return JS_NewFloat64(ctx,
                         JS_VALUE_GET_FLOAT64(x) + JS_VALUE_GET_FLOAT64(y));
  } else {
    JSValue args[] = {x, y};
    if (js_add_slow(ctx, args)) {
      return JS_EXCEPTION;
    }
    return args[0];
  }
}

// JSValue anode_js_sub_any(JSContext *ctx, JSValue x, JSValue y) {}

// JSValue anode_js_mul_any(JSContext *ctx, JSValue x, JSValue y) {}

// JSValue anode_js_div_any(JSContext *ctx, JSValue x, JSValue y) {}
