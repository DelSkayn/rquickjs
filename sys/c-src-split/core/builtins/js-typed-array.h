/*
 * QuickJS Javascript Engine
 *
 * Copyright (c) 2017-2021 Fabrice Bellard
 * Copyright (c) 2017-2021 Charlie Gordon
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

#ifndef QUICKJS_JS_TYPED_ARRAY_H
#define QUICKJS_JS_TYPED_ARRAY_H

#include "../types.h"
#include "quickjs/cutils.h"
#include "quickjs/quickjs.h"

JSValue js_array_buffer_constructor3(JSContext* ctx, JSValueConst new_target, uint64_t len, JSClassID class_id, uint8_t* buf, JSFreeArrayBufferDataFunc* free_func, void* opaque, BOOL alloc_flag);
JSValue js_typed_array_constructor(JSContext* ctx, JSValueConst new_target, int argc, JSValueConst* argv, int classid);

JSArrayBuffer* js_get_array_buffer(JSContext* ctx, JSValueConst obj);
BOOL typed_array_is_detached(JSContext* ctx, JSObject* p);
JSValue JS_ThrowTypeErrorDetachedArrayBuffer(JSContext* ctx);

#endif