#ifndef ANODE_EXT_H
#define ANODE_EXT_H
#include "quickjs-internals.h"

JSValue anode_js_add_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_sub_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_mul_any(JSContext *ctx, JSValue x, JSValue y);
JSValue anode_js_div_any(JSContext *ctx, JSValue x, JSValue y);

#endif
